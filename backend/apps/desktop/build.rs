use tauri_build::{AppManifest, Attributes};

/// The commands the dashboard (a remote page: the daemon's address) may call besides the plugin's.
/// Each gets a permission named `allow-<command with dashes>` for `capabilities/`.
const APP_COMMANDS: &[&str] = &["hide_window", "recall_register", "recall_status"];

fn main() {
    // `build` panics with the reason when it fails, which is how a build script reports an error.
    tauri_build::try_build(Attributes::new().app_manifest(AppManifest::new().commands(APP_COMMANDS))).unwrap_or_else(|e| {
        eprintln!("tauri-build failed: {e:#}");
        std::process::exit(1);
    });
}
