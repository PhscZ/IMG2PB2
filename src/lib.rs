//! IMG2PB2 — shared application core that compiles both natively and to WASM.
//!
//! - Native (`src/main.rs`): `eframe::run_native` with a background thread that
//!   appends objects to the selected PB2 XML file on disk.
//! - Web (`web-pkg` crate, `img2pb2_web::WebHandle`): `eframe::WebRunner`. Files
//!   are read into memory and the result is offered as a browser download.

use std::io::Write;

use eframe::egui::{self, Color32, Frame, RichText, Rounding, Stroke, TextureHandle, Vec2};

#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs,
    io::{BufWriter, Read},
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

#[cfg(not(target_arch = "wasm32"))]
use rfd::FileDialog;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
const MAX_VIEWER_BYTES: u64 = 2 * 1024 * 1024;
const MAX_IMAGE_PIXELS: u64 = 80_000_000;

// ---------------------------------------------------------------------------
// Public re-exports / entry helpers
// ---------------------------------------------------------------------------

/// Decode an embedded `.ico` into the RGBA pixels eframe wants for the window
/// title-bar icon. Native only (web has no window icon).
#[cfg(not(target_arch = "wasm32"))]
pub fn load_icon_data(bytes: &[u8]) -> egui::IconData {
    match image::load_from_memory_with_format(bytes, image::ImageFormat::Ico) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            egui::IconData {
                rgba: rgba.into_raw(),
                width: w,
                height: h,
            }
        }
        Err(_) => egui::IconData {
            rgba: vec![0; 4],
            width: 1,
            height: 1,
        },
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct Pb2ImgApp {
    /// Decoded source image, kept in memory so the same buffer feeds the preview
    /// and the conversion on every platform.
    image: Option<image::RgbaImage>,
    image_name: String,
    xml_content: String,
    xml_name: String,

    pixel_x_size: String,
    pixel_y_size: String,
    x_position: String,
    y_position: String,
    background: String,
    x_offset: String,
    y_offset: String,
    attach_to: String,
    draw_in_front: bool,
    spawn_shadows: bool,
    option: InsertOption,

    x_progress: f32,
    y_progress: f32,
    processing: bool,
    preview: Option<TextureHandle>,
    status: String,

    // Native: background worker + output path.
    #[cfg(not(target_arch = "wasm32"))]
    worker: Option<Receiver<WorkerMessage>>,
    #[cfg(not(target_arch = "wasm32"))]
    xml_path: Option<PathBuf>,

    // Web: channels filled by async file pickers and drained each frame.
    #[cfg(target_arch = "wasm32")]
    pending_image: Rc<RefCell<Option<Result<(image::RgbaImage, String), String>>>>,
    #[cfg(target_arch = "wasm32")]
    pending_xml: Rc<RefCell<Option<(String, String)>>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InsertOption {
    Basic,
    Vertical,
    Horizontal,
    TwoDimensional,
    MultilayerOneDimensional,
    MultilayerTwoDimensional,
}

impl InsertOption {
    fn label(self) -> &'static str {
        match self {
            Self::Basic => "BASIC",
            Self::Vertical => "VERTICAL",
            Self::Horizontal => "HORIZONTAL",
            Self::TwoDimensional => "TWO DIMENSIONAL",
            Self::MultilayerOneDimensional => "MULTILAYER 1D",
            Self::MultilayerTwoDimensional => "MULTILAYER 2D",
        }
    }
}

impl Pb2ImgApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.visuals.panel_fill = Color32::from_rgb(18, 22, 33);
        style.visuals.extreme_bg_color = Color32::from_rgb(27, 33, 48);
        style.visuals.faint_bg_color = Color32::from_rgb(24, 29, 43);
        style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(24, 29, 43);
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(37, 45, 64);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(48, 59, 82);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(64, 78, 109);
        style.visuals.widgets.inactive.fg_stroke =
            Stroke::new(1.0_f32, Color32::from_rgb(222, 229, 240));
        style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        cc.egui_ctx.set_style(style);

        Self {
            image: None,
            image_name: String::new(),
            xml_content: String::new(),
            xml_name: String::new(),
            pixel_x_size: "10".into(),
            pixel_y_size: "10".into(),
            x_position: "0".into(),
            y_position: "0".into(),
            background: "0".into(),
            x_offset: "0".into(),
            y_offset: "0".into(),
            attach_to: String::new(),
            draw_in_front: true,
            spawn_shadows: false,
            option: InsertOption::Basic,
            x_progress: 0.0,
            y_progress: 0.0,
            processing: false,
            preview: None,
            status: "Select an image and a PB2 XML file to begin.".into(),

            #[cfg(not(target_arch = "wasm32"))]
            worker: None,
            #[cfg(not(target_arch = "wasm32"))]
            xml_path: None,

            #[cfg(target_arch = "wasm32")]
            pending_image: Rc::new(RefCell::new(None)),
            #[cfg(target_arch = "wasm32")]
            pending_xml: Rc::new(RefCell::new(None)),
        }
    }

    fn settings(&self) -> Result<InsertSettings, String> {
        let parse_f64 = |label: &str, value: &str| {
            value
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("{label} must be a number."))
        };

        let pixel_width = parse_f64("Pixel X size", &self.pixel_x_size)?;
        let pixel_height = parse_f64("Pixel Y size", &self.pixel_y_size)?;
        if pixel_width <= 0.0 || pixel_height <= 0.0 {
            return Err("Pixel X size and Pixel Y size must be greater than zero.".into());
        }

        let material = parse_material(&self.background)?;
        let is_material_3 = material == "3";
        let material_xml = xml_escape(&material);
        let attach_xml = if self.attach_to.trim().is_empty() {
            String::new()
        } else {
            format!(" a=\"{}\"", xml_escape(self.attach_to.trim()))
        };

        Ok(InsertSettings {
            pixel_width,
            pixel_height,
            x_position: parse_f64("X position", &self.x_position)?,
            y_position: parse_f64("Y position", &self.y_position)?,
            material_xml,
            is_material_3,
            x_offset: parse_f64("X offset", &self.x_offset)?,
            y_offset: parse_f64("Y offset", &self.y_offset)?,
            attach_xml,
            draw_in_front: self.draw_in_front,
            spawn_shadows: self.spawn_shadows,
        })
    }

    fn required_fields_are_defined(&self) -> bool {
        self.image.is_some() && !self.xml_name.trim().is_empty() && self.settings().is_ok()
    }

    // -- file selection -----------------------------------------------------

    #[cfg(not(target_arch = "wasm32"))]
    fn select_image(&mut self, ctx: &egui::Context) {
        let Some(path) = FileDialog::new()
            .add_filter("Image files", &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
            .pick_file()
        else {
            return;
        };

        match image::image_dimensions(&path) {
            Ok((width, height)) if width as u64 * height as u64 > MAX_IMAGE_PIXELS => {
                self.status = format!(
                    "Image is {width}×{height}; the maximum supported size is {} pixels.",
                    MAX_IMAGE_PIXELS
                );
                return;
            }
            Err(error) => {
                self.status = format!("Could not read image dimensions: {error}");
                return;
            }
            _ => {}
        }

        match image::open(&path) {
            Ok(image) => {
                let rgba = image.to_rgba8();
                self.image_name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.preview = Some(load_texture_from_image(ctx, &rgba));
                self.image = Some(rgba);
                self.status = "Image loaded successfully.".into();
            }
            Err(error) => self.status = format!("Could not load image: {error}"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn select_xml(&mut self, _ctx: &egui::Context) {
        if let Some(path) = FileDialog::new()
            .add_filter("PB2 XML files", &["xml", "txt"])
            .pick_file()
        {
            self.xml_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.xml_path = Some(path.clone());
            match read_viewer_content(&path) {
                Ok(content) => {
                    self.xml_content = content;
                    self.status = "PB2 XML file loaded.".into();
                }
                Err(error) => {
                    self.xml_content.clear();
                    self.status = format!("Could not read PB2 XML file: {error}");
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn select_image(&mut self, ctx: &egui::Context) {
        let pending = self.pending_image.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let handle = rfd::AsyncFileDialog::new()
                .add_filter("Image files", &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
                .pick_file()
                .await;
            if let Some(handle) = handle {
                let name = handle.file_name();
                let bytes = handle.read().await;
                match image::load_from_memory(&bytes) {
                    Ok(image) => *pending.borrow_mut() = Some(Ok((image.to_rgba8(), name))),
                    Err(error) => {
                        *pending.borrow_mut() = Some(Err(format!("Could not load image: {error}")))
                    }
                }
                ctx.request_repaint();
            }
        });
    }

    #[cfg(target_arch = "wasm32")]
    fn select_xml(&mut self, ctx: &egui::Context) {
        let pending = self.pending_xml.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let handle = rfd::AsyncFileDialog::new()
                .add_filter("PB2 XML files", &["xml", "txt"])
                .pick_file()
                .await;
            if let Some(handle) = handle {
                let name = handle.file_name();
                let bytes = handle.read().await;
                let content = String::from_utf8_lossy(&bytes).into_owned();
                *pending.borrow_mut() = Some((content, name));
                ctx.request_repaint();
            }
        });
    }

    #[cfg(target_arch = "wasm32")]
    fn drain_pending(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.pending_image.borrow_mut().take() {
            match result {
                Ok((rgba, name)) => {
                    let (w, h) = rgba.dimensions();
                    if w as u64 * h as u64 > MAX_IMAGE_PIXELS {
                        self.status = format!(
                            "Image is {w}×{h}; the maximum supported size is {} pixels.",
                            MAX_IMAGE_PIXELS
                        );
                    } else {
                        self.preview = Some(load_texture_from_image(ctx, &rgba));
                        self.image = Some(rgba);
                        self.image_name = name;
                        self.status = "Image loaded successfully.".into();
                    }
                }
                Err(error) => self.status = error,
            }
        }

        if let Some((content, name)) = self.pending_xml.borrow_mut().take() {
            self.xml_content = content;
            self.xml_name = name;
            self.status = "PB2 XML file loaded.".into();
        }
    }

    // -- conversion ---------------------------------------------------------

    #[cfg(not(target_arch = "wasm32"))]
    fn insert_image(&mut self) {
        let Some(image) = self.image.clone() else {
            self.status = "Select an image first.".into();
            return;
        };
        let settings = match self.settings() {
            Ok(settings) => settings,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        let Some(xml_path) = self.xml_path.clone() else {
            self.status = "Select a PB2 XML file first.".into();
            return;
        };
        let option = self.option;
        let (sender, receiver) = mpsc::channel();

        self.processing = true;
        self.worker = Some(receiver);
        self.x_progress = 0.0;
        self.y_progress = 0.0;
        self.status = "Loading image and starting conversion…".into();
        thread::spawn(move || {
            convert_image_in_background(image, xml_path, settings, option, sender)
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_worker(&mut self) {
        let Some(receiver) = &self.worker else {
            return;
        };
        loop {
            match receiver.try_recv() {
                Ok(WorkerMessage::Progress { x, y }) => {
                    self.x_progress = x;
                    self.y_progress = y;
                }
                Ok(WorkerMessage::Finished { count, path }) => {
                    self.processing = false;
                    self.worker = None;
                    self.x_progress = 1.0;
                    self.y_progress = 1.0;
                    self.xml_content = read_viewer_content(&path).unwrap_or_else(|error| {
                        format!("Objects were appended, but the viewer could not refresh: {error}")
                    });
                    self.status = format!("Appended {count} background object(s).");
                    break;
                }
                Ok(WorkerMessage::Failed(error)) => {
                    self.processing = false;
                    self.worker = None;
                    self.status = error;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.processing = false;
                    self.worker = None;
                    self.status = "Conversion worker stopped unexpectedly.".into();
                    break;
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn insert_image(&mut self) {
        let settings = match self.settings() {
            Ok(settings) => settings,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        let option = self.option;
        let mut output: Vec<u8> = self.xml_content.as_bytes().to_vec();

        self.processing = true;
        self.status = "Converting…".into();

        let result = {
            let Some(image) = self.image.as_ref() else {
                self.status = "Select an image first.".into();
                self.processing = false;
                return;
            };
            run_conversion(image, &settings, option, &mut output, &|_, _| {})
        };

        match result {
            Ok(count) => {
                self.xml_content = String::from_utf8_lossy(&output).into_owned();
                self.x_progress = 1.0;
                self.y_progress = 1.0;
                self.status = format!("Appended {count} object(s). Downloading the result…");
                let name = if self.xml_name.is_empty() {
                    "pb2_output.xml".to_string()
                } else {
                    self.xml_name.clone()
                };
                download(&name, &output);
            }
            Err(error) => self.status = format!("Conversion failed: {error}"),
        }
        self.processing = false;
    }
}

impl eframe::App for Pb2ImgApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.poll_worker();
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.drain_pending(ctx);
        }
        if self.processing {
            ctx.request_repaint();
        }

        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(Color32::from_rgb(18, 22, 33))
                    .inner_margin(Vec2::new(20.0, 16.0)),
            )
            .show(ctx, |ui| {
                const MIN_PREVIEW_SIZE: f32 = 500.0;
                const MIN_CONTROLS_WIDTH: f32 = 550.0;
                const PREVIEW_HEADER_HEIGHT: f32 = 18.0;
                const COLUMN_GAP: f32 = 10.0;
                let preview_size = (ui.available_height() - PREVIEW_HEADER_HEIGHT)
                    .min(ui.available_width() - MIN_CONTROLS_WIDTH - COLUMN_GAP)
                    .max(MIN_PREVIEW_SIZE);
                let controls_width = ui.available_width() - preview_size - COLUMN_GAP;

                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(controls_width, ui.available_height()),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.heading(
                                RichText::new("IMG2PB2")
                                    .size(27.0)
                                    .strong()
                                    .color(Color32::from_rgb(123, 181, 255)),
                            );
                            ui.add_space(5.0);
                            source_files(ui, self, ctx, controls_width);
                            ui.add_space(16.0);
                            placement_controls(ui, self, controls_width);
                            ui.add_space(16.0);
                            option_controls(ui, self, controls_width);
                            ui.add_space(16.0);
                            ui.horizontal(|ui| {
                                let can_insert =
                                    !self.processing && self.required_fields_are_defined();
                                if ui
                                    .add_enabled_ui(can_insert, |ui| {
                                        ui.add_sized(
                                            [150.0, 38.0],
                                            dark_button(
                                                RichText::new("INSERT IMAGE")
                                                    .strong()
                                                    .color(label_color()),
                                            ),
                                        )
                                    })
                                    .inner
                                    .clicked()
                                {
                                    self.insert_image();
                                }
                                ui.label(
                                    RichText::new(&self.status)
                                        .color(Color32::from_rgb(161, 188, 222)),
                                );
                                if self.processing {
                                    ui.spinner();
                                }
                                ui.label(
                                    RichText::new(format!(
                                        "X {:>3.0}%  Y {:>3.0}%",
                                        self.x_progress * 100.0,
                                        self.y_progress * 100.0
                                    ))
                                    .color(Color32::from_rgb(161, 188, 222)),
                                );
                            });
                            ui.add_space(10.0);
                            xml_content_viewer(ui, &mut self.xml_content, controls_width);
                        },
                    );
                    ui.add_space(COLUMN_GAP);
                    preview_panel(ui, self, preview_size);
                });
            });
    }
}

// ---------------------------------------------------------------------------
// UI helpers
// ---------------------------------------------------------------------------

fn source_files(ui: &mut egui::Ui, app: &mut Pb2ImgApp, ctx: &egui::Context, controls_width: f32) {
    const BUTTON_WIDTH: f32 = 128.0;
    let path_width = controls_width - BUTTON_WIDTH - ui.spacing().item_spacing.x;
    section_title(ui, "SOURCE FILES");
    ui.horizontal(|ui| {
        if ui
            .add_sized(
                [BUTTON_WIDTH, 34.0],
                dark_button(RichText::new("Select image").color(label_color())),
            )
            .clicked()
        {
            app.select_image(ctx);
        }
        ui.add_sized(
            [path_width, 34.0],
            dark_text_edit(&mut app.image_name).hint_text("No image selected"),
        );
    });
    ui.horizontal(|ui| {
        if ui
            .add_sized(
                [BUTTON_WIDTH, 34.0],
                dark_button(RichText::new("Select PB2 XML").color(label_color())),
            )
            .clicked()
        {
            app.select_xml(ctx);
        }
        ui.add_sized(
            [path_width, 34.0],
            dark_text_edit(&mut app.xml_name).hint_text("No XML or text file selected"),
        );
    });
}

fn placement_controls(ui: &mut egui::Ui, app: &mut Pb2ImgApp, controls_width: f32) {
    const COLUMN_GAP: f32 = 16.0;
    let field_width = (controls_width - COLUMN_GAP) / 2.0;

    section_title(ui, "PLACEMENT");
    egui::Grid::new("placement_grid")
        .num_columns(2)
        .spacing([COLUMN_GAP, 12.0])
        .show(ui, |ui| {
            placement_field(ui, "Pixel X size", &mut app.pixel_x_size, field_width);
            placement_field(ui, "Pixel Y size", &mut app.pixel_y_size, field_width);
            ui.end_row();
            placement_field(ui, "X position", &mut app.x_position, field_width);
            placement_field(ui, "Y position", &mut app.y_position, field_width);
            ui.end_row();
            material_field(ui, &mut app.background, field_width);
            placement_field(ui, "Attach to", &mut app.attach_to, field_width);
            ui.end_row();
            placement_field(ui, "X offset", &mut app.x_offset, field_width);
            placement_field(ui, "Y offset", &mut app.y_offset, field_width);
            ui.end_row();
        });
}

fn placement_field(ui: &mut egui::Ui, label: &str, value: &mut String, field_width: f32) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).color(label_color()));
        ui.add_sized([field_width, 26.0], dark_text_edit(value));
    });
}

fn material_field(ui: &mut egui::Ui, value: &mut String, field_width: f32) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Background").color(label_color()));
            ui.label(
                RichText::new(format!("Material: {}", material_name(value)))
                    .small()
                    .color(Color32::from_rgb(142, 165, 198)),
            );
        });
        ui.add_sized([field_width, 26.0], dark_text_edit(value));
    });
}

fn label_color() -> Color32 {
    Color32::from_rgb(190, 202, 221)
}

fn dark_button<'a>(label: impl Into<egui::WidgetText>) -> egui::Button<'a> {
    egui::Button::new(label)
        .fill(Color32::from_rgb(38, 47, 67))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(78, 94, 124)))
}

fn dark_text_edit(value: &mut String) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(value)
        .background_color(Color32::from_rgb(28, 35, 51))
        .text_color(Color32::from_rgb(232, 237, 247))
        .vertical_align(egui::Align::Center)
}

fn xml_content_viewer(ui: &mut egui::Ui, content: &mut String, controls_width: f32) {
    let viewer_height = ui.available_height().max(80.0);
    ui.allocate_ui(Vec2::new(controls_width, viewer_height), |ui| {
        Frame::none()
            .fill(Color32::from_rgb(22, 28, 42))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(78, 94, 124)))
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(controls_width - 2.0, viewer_height - 2.0));
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if content.is_empty() {
                            ui.label(
                                RichText::new(
                                    "Selected PB2 XML or text file contents will appear here.",
                                )
                                .monospace()
                                .color(Color32::GRAY),
                            );
                        } else {
                            ui.label(
                                RichText::new(content.as_str())
                                    .monospace()
                                    .color(Color32::from_rgb(214, 223, 238)),
                            );
                        }
                    });
            });
    });
}

fn option_controls(ui: &mut egui::Ui, app: &mut Pb2ImgApp, controls_width: f32) {
    section_title(ui, "INSERT MODE");
    ui.horizontal_wrapped(|ui| {
        for option in [
            InsertOption::Basic,
            InsertOption::Vertical,
            InsertOption::Horizontal,
            InsertOption::TwoDimensional,
            InsertOption::MultilayerOneDimensional,
            InsertOption::MultilayerTwoDimensional,
        ] {
            ui.radio_value(
                &mut app.option,
                option,
                RichText::new(option.label()).color(label_color()),
            );
        }
    });
    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.draw_in_front,
            RichText::new("DRAW IN FRONT").color(label_color()),
        );
        ui.checkbox(
            &mut app.spawn_shadows,
            RichText::new("SPAWN SHADOWS").color(label_color()),
        );
    });
    ui.add_space(8.0);
    ui.allocate_ui_with_layout(
        Vec2::new(controls_width, 30.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            const BAR_GAP: f32 = 4.0;
            const LABEL_WIDTH: f32 = 10.0;
            ui.spacing_mut().item_spacing.x = BAR_GAP;
            let bar_width = (controls_width - 2.0 * LABEL_WIDTH - 3.0 * BAR_GAP) / 2.0;
            ui.label("X");
            ui.add_sized(
                [bar_width, 22.0],
                egui::ProgressBar::new(app.x_progress).show_percentage(),
            );
            ui.label("Y");
            ui.add_sized(
                [bar_width, 22.0],
                egui::ProgressBar::new(app.y_progress).show_percentage(),
            );
        },
    );
}

fn preview_panel(ui: &mut egui::Ui, app: &Pb2ImgApp, preview_size: f32) {
    const FRAME_MARGIN: f32 = 14.0;
    const FRAME_STROKE: f32 = 1.0;
    let canvas_size = Vec2::splat(preview_size - 2.0 * (FRAME_MARGIN + FRAME_STROKE));

    ui.vertical(|ui| {
        section_title(ui, "IMAGE PREVIEW");
        ui.add_space(4.0);
        ui.allocate_ui(Vec2::splat(preview_size), |ui| {
            Frame::none()
                .fill(Color32::from_rgb(22, 28, 42))
                .stroke(Stroke::new(FRAME_STROKE, Color32::from_rgb(61, 73, 101)))
                .rounding(Rounding::same(10.0))
                .inner_margin(FRAME_MARGIN)
                .show(ui, |ui| {
                    ui.set_min_size(canvas_size);
                    ui.set_max_size(canvas_size);
                    ui.centered_and_justified(|ui| {
                        if let Some(texture) = &app.preview {
                            let image_size = texture.size_vec2();
                            let scale =
                                (canvas_size.x / image_size.x).min(canvas_size.y / image_size.y);
                            ui.image((texture.id(), image_size * scale));
                        } else {
                            ui.label(
                                RichText::new("Your selected image\nwill appear here")
                                    .size(17.0)
                                    .color(Color32::GRAY),
                            );
                        }
                    });
                });
        });
    });
}

fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(
        RichText::new(title)
            .size(12.0)
            .strong()
            .color(Color32::from_rgb(123, 181, 255)),
    );
}

fn load_texture_from_image(ctx: &egui::Context, image: &image::RgbaImage) -> TextureHandle {
    let size = [image.width() as usize, image.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    ctx.load_texture(
        "image_preview",
        color_image,
        egui::TextureOptions::default(),
    )
}

// ---------------------------------------------------------------------------
// Conversion core (platform-agnostic)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct InsertSettings {
    pixel_width: f64,
    pixel_height: f64,
    x_position: f64,
    y_position: f64,
    material_xml: String,
    is_material_3: bool,
    x_offset: f64,
    y_offset: f64,
    attach_xml: String,
    draw_in_front: bool,
    spawn_shadows: bool,
}

#[derive(Clone, Copy)]
struct BackgroundRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 3],
}

fn parse_material(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.starts_with('c') && value.len() > 1 {
        return Ok(value.to_owned());
    }
    let material = value.parse::<u8>().map_err(|_| {
        "Background must be a material number or custom value starting with c.".to_owned()
    })?;
    if material > 16 {
        return Err("Background material must be between 0 and 16.".into());
    }
    Ok(material.to_string())
}

fn material_name(value: &str) -> &'static str {
    match value.trim() {
        "0" => "basic",
        "1" => "ground",
        "2" => "usurpation",
        "3" => "white",
        "4" => "elevator path",
        "5" => "impure canal",
        "6" => "red",
        "7" => "green",
        "8" => "blue",
        "9" => "damned",
        "10" => "panel default",
        "11" => "panel bright",
        "12" => "panel dark",
        "13" => "rocks",
        "14" => "pixel wall",
        "15" => "pixel background",
        "16" => "pixel open door",
        value if value.starts_with('c') => "custom background",
        _ => "invalid material",
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('\"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Append the converted image into any writer. Returns the number of `<bg>`
/// objects written. The progress callback receives `(x, y)` in `0.0..=1.0`.
fn run_conversion<W: Write>(
    image: &image::RgbaImage,
    settings: &InsertSettings,
    option: InsertOption,
    output: &mut W,
    progress: &impl Fn(f32, f32),
) -> std::io::Result<u64> {
    output.write_all(b"\n")?;
    let mut count = 0_u64;
    match option {
        InsertOption::Basic => write_basic(image, settings, output, progress, &mut count)?,
        InsertOption::Vertical => write_grouped(
            image,
            settings,
            output,
            progress,
            &mut count,
            InsertOption::Vertical,
        )?,
        InsertOption::Horizontal => {
            write_horizontal(image, settings, output, progress, &mut count)?
        }
        InsertOption::TwoDimensional => write_grouped(
            image,
            settings,
            output,
            progress,
            &mut count,
            InsertOption::TwoDimensional,
        )?,
        InsertOption::MultilayerTwoDimensional => write_grouped(
            image,
            settings,
            output,
            progress,
            &mut count,
            InsertOption::MultilayerTwoDimensional,
        )?,
        InsertOption::MultilayerOneDimensional => write_grouped(
            image,
            settings,
            output,
            progress,
            &mut count,
            InsertOption::MultilayerOneDimensional,
        )?,
    }
    output.flush()?;
    Ok(count)
}

fn write_basic<W: Write>(
    image: &image::RgbaImage,
    settings: &InsertSettings,
    output: &mut W,
    progress: &impl Fn(f32, f32),
    count: &mut u64,
) -> std::io::Result<()> {
    for y in 0..image.height() {
        for x in 0..image.width() {
            let color = image.get_pixel(x, y).0;
            if color[3] != 0 {
                write_rect(
                    output,
                    BackgroundRect {
                        x,
                        y,
                        width: 1,
                        height: 1,
                        color: [color[0], color[1], color[2]],
                    },
                    settings,
                )?;
                *count += 1;
            }
            if x % 4_096 == 0 {
                report(progress, x, y, image.width(), image.height());
            }
        }
        report(progress, image.width(), y, image.width(), image.height());
    }
    Ok(())
}

fn write_horizontal<W: Write>(
    image: &image::RgbaImage,
    settings: &InsertSettings,
    output: &mut W,
    progress: &impl Fn(f32, f32),
    count: &mut u64,
) -> std::io::Result<()> {
    for y in 0..image.height() {
        let mut x = 0;
        while x < image.width() {
            let color = image.get_pixel(x, y).0;
            if color[3] == 0 {
                x += 1;
                continue;
            }
            let mut run = 1;
            while x + run < image.width() && image.get_pixel(x + run, y).0 == color {
                run += 1;
            }
            write_rect(
                output,
                BackgroundRect {
                    x,
                    y,
                    width: run,
                    height: 1,
                    color: [color[0], color[1], color[2]],
                },
                settings,
            )?;
            *count += 1;
            x += run;
            if x % 4_096 == 0 {
                report(progress, x, y, image.width(), image.height());
            }
        }
        report(progress, image.width(), y, image.width(), image.height());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_grouped<W: Write>(
    image: &image::RgbaImage,
    settings: &InsertSettings,
    output: &mut W,
    progress: &impl Fn(f32, f32),
    count: &mut u64,
    option: InsertOption,
) -> std::io::Result<()> {
    let width = image.width();
    let height = image.height();
    let mut covered = vec![false; width as usize * height as usize];
    let color_at = |x: u32, y: u32| image.get_pixel(x, y).0;

    for y in 0..height {
        for x in 0..width {
            let index = (y as usize) * width as usize + x as usize;
            let color = color_at(x, y);
            if covered[index] || color[3] == 0 {
                continue;
            }
            let (rect_width, rect_height) = match option {
                InsertOption::Vertical => {
                    vertical_run(x, y, width, height, color, &covered, &color_at)
                }
                InsertOption::TwoDimensional | InsertOption::MultilayerTwoDimensional => {
                    largest_rectangle(x, y, width, height, color, &covered, &color_at)
                }
                InsertOption::MultilayerOneDimensional => {
                    longest_one_dimensional_run(x, y, width, height, color, &covered, &color_at)
                }
                _ => (1, 1),
            };
            for rect_y in y..y + rect_height {
                let row_start = rect_y as usize * width as usize;
                for rect_x in x..x + rect_width {
                    covered[row_start + rect_x as usize] = true;
                }
            }
            write_rect(
                output,
                BackgroundRect {
                    x,
                    y,
                    width: rect_width,
                    height: rect_height,
                    color: [color[0], color[1], color[2]],
                },
                settings,
            )?;
            *count += 1;
            if x % 4_096 == 0 {
                report(progress, x, y, width, height);
            }
        }
        report(progress, width, y, width, height);
    }
    Ok(())
}

fn same_visible_color(a: [u8; 4], b: [u8; 4]) -> bool {
    a[3] != 0 && b[3] != 0 && a[..3] == b[..3]
}

fn write_rect<W: Write>(
    output: &mut W,
    rect: BackgroundRect,
    settings: &InsertSettings,
) -> std::io::Result<()> {
    let x = settings.x_position + rect.x as f64 * settings.pixel_width;
    let y = settings.y_position + rect.y as f64 * settings.pixel_height;
    let w = rect.width as f64 * settings.pixel_width;
    let h = rect.height as f64 * settings.pixel_height;

    // Material 3 uses raw RGB; all other materials encode half the source RGB
    // (the PB2 renderer applies a 2x brightness multiplier).
    let (c0, c1, c2) = if settings.is_material_3 {
        (rect.color[0], rect.color[1], rect.color[2])
    } else {
        (rect.color[0] / 2, rect.color[1] / 2, rect.color[2] / 2)
    };

    writeln!(
        output,
        "<bg x=\"{x}\" y=\"{y}\" w=\"{w}\" h=\"{h}\" m=\"{}\" c=\"#{:02X}{:02X}{:02X}\"{} u=\"{}\" v=\"{}\" f=\"{}\" s=\"{}\" />",
        settings.material_xml,
        c0, c1, c2,
        settings.attach_xml,
        settings.x_offset,
        settings.y_offset,
        u8::from(settings.draw_in_front),
        settings.spawn_shadows,
    )
}

fn report(progress: &impl Fn(f32, f32), x: u32, y: u32, width: u32, height: u32) {
    progress(x as f32 / width as f32, (y + 1) as f32 / height as f32);
}

#[allow(clippy::too_many_arguments)]
fn vertical_run(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
    covered: &[bool],
    color_at: &impl Fn(u32, u32) -> [u8; 4],
) -> (u32, u32) {
    let mut run_height = 1;
    while y + run_height < height
        && !covered[((y + run_height) * width + x) as usize]
        && same_visible_color(color_at(x, y + run_height), color)
    {
        run_height += 1;
    }
    (1, run_height)
}

#[allow(clippy::too_many_arguments)]
fn horizontal_run(
    x: u32,
    y: u32,
    width: u32,
    color: [u8; 4],
    covered: &[bool],
    color_at: &impl Fn(u32, u32) -> [u8; 4],
) -> (u32, u32) {
    let mut run_width = 1;
    while x + run_width < width
        && !covered[(y * width + x + run_width) as usize]
        && same_visible_color(color_at(x + run_width, y), color)
    {
        run_width += 1;
    }
    (run_width, 1)
}

#[allow(clippy::too_many_arguments)]
fn largest_rectangle(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
    covered: &[bool],
    color_at: &impl Fn(u32, u32) -> [u8; 4],
) -> (u32, u32) {
    let mut best = (1, 1);
    let mut max_width = u32::MAX;
    for rect_y in y..height {
        let mut row_width = 0;
        while row_width < max_width
            && x + row_width < width
            && !covered[(rect_y * width + x + row_width) as usize]
            && same_visible_color(color_at(x + row_width, rect_y), color)
        {
            row_width += 1;
        }
        if row_width < max_width {
            max_width = row_width;
        }
        if max_width == 0 {
            break;
        }
        let candidate = (max_width, rect_y - y + 1);
        if candidate.0 * candidate.1 > best.0 * best.1 {
            best = candidate;
        }
    }
    best
}

#[allow(clippy::too_many_arguments)]
fn longest_one_dimensional_run(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
    covered: &[bool],
    color_at: &impl Fn(u32, u32) -> [u8; 4],
) -> (u32, u32) {
    let horizontal = horizontal_run(x, y, width, color, covered, color_at);
    let vertical = vertical_run(x, y, width, height, color, covered, color_at);
    if horizontal.0 >= vertical.1 {
        horizontal
    } else {
        vertical
    }
}

// ---------------------------------------------------------------------------
// Native-only: file viewer + background worker
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
enum WorkerMessage {
    Progress { x: f32, y: f32 },
    Finished { count: u64, path: String },
    Failed(String),
}

#[cfg(not(target_arch = "wasm32"))]
fn read_viewer_content(path: impl AsRef<std::path::Path>) -> Result<String, std::io::Error> {
    let path = path.as_ref();
    let file_size = fs::metadata(path)?.len();
    let file = fs::File::open(path)?;
    let mut bytes = Vec::with_capacity(file_size.min(MAX_VIEWER_BYTES) as usize);
    file.take(MAX_VIEWER_BYTES).read_to_end(&mut bytes)?;

    let preview = String::from_utf8_lossy(&bytes);
    if file_size <= MAX_VIEWER_BYTES {
        Ok(preview.into_owned())
    } else {
        Ok(format!(
            "{preview}\n\n[Preview truncated at 2 MiB. The selected file is {file_size} bytes.]"
        ))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn convert_image_in_background(
    image: image::RgbaImage,
    xml_path: PathBuf,
    settings: InsertSettings,
    option: InsertOption,
    sender: mpsc::Sender<WorkerMessage>,
) {
    let total_pixels = image.width() as u64 * image.height() as u64;
    if total_pixels > MAX_IMAGE_PIXELS {
        let _ = sender.send(WorkerMessage::Failed(format!(
            "Image exceeds the {}-pixel processing limit. Resize it before converting.",
            MAX_IMAGE_PIXELS
        )));
        return;
    }

    let file = match fs::OpenOptions::new().append(true).open(&xml_path) {
        Ok(file) => file,
        Err(error) => {
            let _ = sender.send(WorkerMessage::Failed(format!(
                "Could not open PB2 XML file for appending: {error}"
            )));
            return;
        }
    };
    let mut output = BufWriter::with_capacity(1024 * 1024, file);

    let progress_sender = sender.clone();
    let progress = move |x: f32, y: f32| {
        let _ = progress_sender.send(WorkerMessage::Progress { x, y });
    };

    match run_conversion(&image, &settings, option, &mut output, &progress) {
        Ok(count) => {
            let _ = sender.send(WorkerMessage::Progress { x: 1.0, y: 1.0 });
            let _ = sender.send(WorkerMessage::Finished {
                count,
                path: xml_path.to_string_lossy().into_owned(),
            });
        }
        Err(error) => {
            let _ = sender.send(WorkerMessage::Failed(format!(
                "Could not write PB2 XML: {error}"
            )));
        }
    }
}

// ---------------------------------------------------------------------------
// Web-only: download helper + eframe entry point
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
fn download(filename: &str, data: &[u8]) {
    use wasm_bindgen::JsCast;
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };

    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(data));
    let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence(&parts) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };

    if let Ok(anchor) = document.create_element("a") {
        let _ = anchor.set_attribute("href", &url);
        let _ = anchor.set_attribute("download", filename);
        if let Some(element) = anchor.dyn_ref::<web_sys::HtmlAnchorElement>() {
            element.click();
        }
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}
