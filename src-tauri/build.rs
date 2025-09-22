fn main() {
    // Set environment variable to skip icon generation
    std::env::set_var("TAURI_BUILD_SKIP_ICON", "1");
    tauri_build::build()
}
