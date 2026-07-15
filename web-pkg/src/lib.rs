//! Web (WASM) entry point for IMG2PB2.
//!
//! This crate is a thin `cdylib` wrapper that exposes a `#[wasm_bindgen]`
//! `WebHandle` to JavaScript. All application logic lives in the `img2pb2`
//! library crate; this crate exists so the `cdylib` crate-type (required by
//! `wasm-bindgen`) does not get built for native targets, where it would
//! produce a native shared library that the mingw linker cannot handle.

#![cfg(target_arch = "wasm32")]

use img2pb2::Pb2ImgApp;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

/// Handle to the running web app, instantiated from JavaScript.
#[derive(Clone)]
#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,
}

#[wasm_bindgen]
impl WebHandle {
    /// Installs a panic hook and the web logger, then returns.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        eframe::WebLogger::init(log::LevelFilter::Debug).ok();
        Self {
            runner: eframe::WebRunner::new(),
        }
    }

    /// Call this once from JavaScript to start the app on the given canvas.
    #[wasm_bindgen]
    pub async fn start(&self, canvas: HtmlCanvasElement) -> Result<(), JsValue> {
        self.runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(Pb2ImgApp::new(cc)))),
            )
            .await
    }
}
