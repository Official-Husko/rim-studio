//! Build script: generates the permission set of the app commands and the Tauri context.
//!
//! The app commands are listed here so that a command that is not in this list cannot be invoked by
//! the webview even if a capability file names it (security and privacy, section 4.1).

fn main() {
    let commands = &["rs_call", "rs_cancel", "rs_info", "rs_commands"];
    let manifest = tauri_build::AppManifest::new().commands(commands);
    let attributes = tauri_build::Attributes::new().app_manifest(manifest);
    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("the Tauri build step failed: {error:#}");
    }
}
