fn main() {
    // The Windows resource compiler only makes sense for native Windows builds
    // (gnu/msvc). Skip it for wasm32 and any other non-Windows target.
    // NOTE: in build scripts `#[cfg(windows)]` matches the *host*, not the
    // target, so we must inspect the target triple ourselves.
    let target = std::env::var("TARGET").unwrap_or_default();
    let is_windows_native = target.ends_with("-windows-gnu") || target.ends_with("-windows-msvc");

    if is_windows_native {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icon.ico");
        res.compile().unwrap();
    }
}
