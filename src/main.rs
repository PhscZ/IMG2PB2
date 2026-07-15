#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    use img2pb2::{load_icon_data, Pb2ImgApp};

    let icon = load_icon_data(include_bytes!("../icon.ico"));

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1_340.0, 700.0])
            .with_min_inner_size([1_340.0, 700.0])
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "IMG2PB2",
        options,
        Box::new(|cc| Ok(Box::new(Pb2ImgApp::new(cc)))),
    )
}

// When checking/building the bin crate for wasm32 there is no native entry
// point — the web app is launched via `img2pb2::WebHandle` in `src/lib.rs`.
#[cfg(target_arch = "wasm32")]
fn main() {}
