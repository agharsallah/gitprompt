// The plugin's commands, by name. Included by `build.rs` (to generate one permission per command)
// and by the crate (to test that `commands::handler` serves exactly these), so the two lists can't
// drift apart.

/// Every command the plugin registers, as invoked: `plugin:gitprompt|<name>`.
pub const COMMANDS: &[&str] = &[
    "status",
    "open_data_folder",
    "reveal_backup",
    "connect",
    "disconnect",
    "enable",
    "disable",
    "rotate_token",
    "compact",
    "export_to_file",
];
