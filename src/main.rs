use std::path::PathBuf;

use eframe::egui::{self, Color32, Frame, RichText, Rounding, Stroke, TextureHandle, Vec2};
use rfd::FileDialog;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1_200.0, 650.0])
            .with_min_inner_size([1_200.0, 650.0]),
        ..Default::default()
    };

    eframe::run_native(
        "IMG2PB2",
        options,
        Box::new(|cc| Ok(Box::new(Pb2ImgApp::new(cc)))),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InsertOption {
    One,
    Two,
    Three,
    Four,
}

impl InsertOption {
    fn label(self) -> &'static str {
        match self {
            Self::One => "OPTION 1",
            Self::Two => "OPTION 2",
            Self::Three => "OPTION 3",
            Self::Four => "OPTION 4",
        }
    }
}

struct Pb2ImgApp {
    image_path: String,
    xml_path: String,
    xml_content: String,
    pixel_x_size: String,
    pixel_y_size: String,
    x_position: String,
    y_position: String,
    background: String,
    x_offset: String,
    y_offset: String,
    attach_to: String,
    option: InsertOption,
    x_progress: f32,
    y_progress: f32,
    preview: Option<TextureHandle>,
    status: String,
}

impl Pb2ImgApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
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
            image_path: String::new(),
            xml_path: String::new(),
            xml_content: String::new(),
            pixel_x_size: "10".into(),
            pixel_y_size: "10".into(),
            x_position: "0".into(),
            y_position: "0".into(),
            background: "0".into(),
            x_offset: "0".into(),
            y_offset: "0".into(),
            attach_to: String::new(),
            option: InsertOption::One,
            x_progress: 0.5,
            y_progress: 0.5,
            preview: None,
            status: "Select an image and a PB2 XML file to begin.".into(),
        }
    }

    fn select_image(&mut self, ctx: &egui::Context) {
        let selected = FileDialog::new()
            .add_filter("Image files", &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
            .pick_file();

        if let Some(path) = selected {
            self.image_path = path.display().to_string();
            match load_texture(ctx, &path) {
                Ok(texture) => {
                    self.preview = Some(texture);
                    self.status = "Image loaded successfully.".into();
                }
                Err(error) => self.status = format!("Could not load image: {error}"),
            }
        }
    }

    fn select_xml(&mut self) {
        if let Some(path) = FileDialog::new()
            .add_filter("PB2 XML files", &["xml", "txt"])
            .pick_file()
        {
            self.xml_path = path.display().to_string();
            match std::fs::read_to_string(&path) {
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

    fn required_fields_are_defined(&self) -> bool {
        [
            &self.image_path,
            &self.xml_path,
            &self.pixel_x_size,
            &self.pixel_y_size,
            &self.x_position,
            &self.y_position,
            &self.background,
            &self.x_offset,
            &self.y_offset,
        ]
        .iter()
        .all(|value| !value.trim().is_empty())
    }

    fn insert_image(&mut self) {
        if !self.required_fields_are_defined() {
            self.status = "Complete all required image and placement fields first.".into();
            return;
        }
        self.status = format!("Ready to insert using {}.", self.option.label());
    }
}

impl eframe::App for Pb2ImgApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
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
                                let can_insert = self.required_fields_are_defined();
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
            dark_text_edit(&mut app.image_path).hint_text("No image selected"),
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
            app.select_xml();
        }
        ui.add_sized(
            [path_width, 34.0],
            dark_text_edit(&mut app.xml_path).hint_text("No XML or text file selected"),
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
            placement_field(ui, "Background", &mut app.background, field_width);
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
    ui.horizontal(|ui| {
        for option in [
            InsertOption::One,
            InsertOption::Two,
            InsertOption::Three,
            InsertOption::Four,
        ] {
            ui.radio_value(
                &mut app.option,
                option,
                RichText::new(option.label()).color(label_color()),
            );
        }
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

fn load_texture(ctx: &egui::Context, path: &PathBuf) -> Result<TextureHandle, String> {
    let image = image::open(path)
        .map_err(|error| error.to_string())?
        .to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Ok(ctx.load_texture(
        "image_preview",
        color_image,
        egui::TextureOptions::default(),
    ))
}
