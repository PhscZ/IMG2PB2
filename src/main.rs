#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    use img2pb2::{load_icon_data, Pb2ImgApp};

    let icon = load_icon_data(include_bytes!("../icon.ico"));

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1_340.0, 680.0])
            .with_min_inner_size([1_340.0, 680.0])
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "IMG2PB2",
        options,
        Box::new(|cc| Ok(Box::new(Pb2ImgApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
