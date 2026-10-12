fn main() {
    // Capabilities live in config/ with the other data files, not beside the
    // manifest where tauri-build looks by default.
    let attributes =
        tauri_build::Attributes::new().capabilities_path_pattern("./config/capabilities/**/*");
    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("{error:#}");
    }
}
