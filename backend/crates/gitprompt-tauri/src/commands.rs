//! The plugin's commands (`plugin:gitprompt|<name>`), each thin over [`Service`] or the state.
//! Arguments are snake_case, like the HTTP API's JSON; errors are [`CommandError`]s. The single
//! handler list is [`handler`]; [`crate::COMMANDS`] names the same commands for the permissions.
//!
//! | command          | arguments                                     | returns         |
//! |------------------|-----------------------------------------------|-----------------|
//! | status           |                                               | [`StatusOut`]   |
//! | open_data_folder |                                               | the folder path |
//! | reveal_backup    | name                                          | the file path   |
//! | connect          | source, dry_run = false                       | `Change`        |
//! | disconnect       | source, dry_run = false                       | `Change`        |
//! | enable / disable | source                                        | `Enabled`       |
//! | rotate_token     | dry_run = true                                | `Rotation`      |
//! | compact          | older_than_days?, dry_run = true              | `ReportOut`     |
//! | export_to_file   | kind, path, query? (`ExportQuery`), filter?   | [`ExportSaved`] |

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{Runtime, State, ipc::Invoke};

use gitprompt_server::model::{Change, CompactIn, Enabled, ExportKind, ExportQuery, ReportOut, Rotation};
use gitprompt_store::PromptFilter;

use crate::error::CommandError;
use crate::native::backup_path;
use crate::state::{DaemonStatus, GitPrompt};

type Answer<T> = Result<T, CommandError>;

/// Every command, in one list.
pub(crate) fn handler<R: Runtime>() -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        status,
        open_data_folder,
        reveal_backup,
        connect,
        disconnect,
        enable,
        disable,
        rotate_token,
        compact,
        export_to_file
    ]
}

/// What the shell shows about GitPrompt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusOut {
    /// `http://127.0.0.1:<port>/`: the dashboard.
    pub url: String,
    pub port: u16,
    /// This plugin's (and the backend's) version.
    pub version: String,
    pub data_dir: String,
    pub daemon: DaemonStatus,
}

/// An export written to disk.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportSaved {
    pub path: String,
    /// The name the server suggests (`gitprompt-<kind>-...-<day>.<ext>`).
    pub filename: String,
    pub media_type: String,
    pub bytes: u64,
}

#[tauri::command(rename_all = "snake_case")]
async fn status(state: State<'_, GitPrompt>) -> Answer<StatusOut> {
    Ok(StatusOut {
        url: state.url(),
        port: state.addr().port(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        data_dir: state.settings().home.display().to_string(),
        daemon: state.status(),
    })
}

/// Open the data directory in the file manager (created if missing). Works whoever runs the daemon.
#[tauri::command(rename_all = "snake_case")]
async fn open_data_folder(state: State<'_, GitPrompt>) -> Answer<String> {
    let home = state.settings().home.clone();
    std::fs::create_dir_all(&home)?;
    state.opener().open(&home)?;
    Ok(home.display().to_string())
}

/// Show one of the config-file backups (`DataOut::backups[].name`) in the file manager.
#[tauri::command(rename_all = "snake_case")]
async fn reveal_backup(state: State<'_, GitPrompt>, name: String) -> Answer<String> {
    let path = backup_path(&state.settings().backups(), &name)?;
    state.opener().reveal(&path)?;
    Ok(path.display().to_string())
}

#[tauri::command(rename_all = "snake_case")]
async fn connect(state: State<'_, GitPrompt>, source: String, dry_run: Option<bool>) -> Answer<Change> {
    Ok(state.service()?.connect(&source, dry_run.unwrap_or(false)).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn disconnect(state: State<'_, GitPrompt>, source: String, dry_run: Option<bool>) -> Answer<Change> {
    Ok(state.service()?.disconnect(&source, dry_run.unwrap_or(false)).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn enable(state: State<'_, GitPrompt>, source: String) -> Answer<Enabled> {
    Ok(state.service()?.enable(&source).await?)
}

#[tauri::command(rename_all = "snake_case")]
async fn disable(state: State<'_, GitPrompt>, source: String) -> Answer<Enabled> {
    Ok(state.service()?.disable(&source).await?)
}

/// Dry run unless `dry_run: false`, like the HTTP route.
#[tauri::command(rename_all = "snake_case")]
async fn rotate_token(state: State<'_, GitPrompt>, dry_run: Option<bool>) -> Answer<Rotation> {
    Ok(state.service()?.rotate_token(dry_run.unwrap_or(true)).await?)
}

/// Dry run unless `dry_run: false`, like the HTTP route.
#[tauri::command(rename_all = "snake_case")]
async fn compact(state: State<'_, GitPrompt>, older_than_days: Option<i64>, dry_run: Option<bool>) -> Answer<ReportOut> {
    Ok(state.service()?.compact(CompactIn { older_than_days }, dry_run.unwrap_or(true)).await?)
}

/// Render an export and write it to `path` (absolute, chosen by the frontend's save dialog; its
/// folder must exist). An existing file is replaced.
#[tauri::command(rename_all = "snake_case")]
async fn export_to_file(
    state: State<'_, GitPrompt>,
    kind: ExportKind,
    path: PathBuf,
    query: Option<ExportQuery>,
    filter: Option<PromptFilter>,
) -> Answer<ExportSaved> {
    check_target(&path)?;
    let file = state.service()?.export(kind, query.unwrap_or_default(), filter.unwrap_or_default()).await?;
    let bytes = file.body.len() as u64;
    let target = path.clone();
    tauri::async_runtime::spawn_blocking(move || std::fs::write(&target, &file.body).map(|()| (file.filename, file.media_type)))
        .await
        .map_err(|e| CommandError::internal(format!("write task failed: {e}")))?
        .map(|(filename, media_type)| ExportSaved { path: path.display().to_string(), filename, media_type, bytes })
        .map_err(|e| CommandError::internal(format!("cannot write {}: {e}", path.display())))
}

/// An export target: an absolute path in an existing folder, not a folder itself.
fn check_target(path: &Path) -> Answer<()> {
    if !path.is_absolute() {
        return Err(CommandError::invalid(format!("{} is not an absolute path", path.display())));
    }
    if path.is_dir() {
        return Err(CommandError::invalid(format!("{} is a folder", path.display())));
    }
    match path.parent() {
        Some(parent) if parent.is_dir() => Ok(()),
        _ => Err(CommandError::not_found(format!("the folder of {} does not exist", path.display()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_targets_are_absolute_files_in_existing_folders() {
        let dir = tempfile::tempdir().unwrap();
        assert!(check_target(&dir.path().join("out.html")).is_ok());
        assert_eq!(check_target(Path::new("out.html")).unwrap_err().status, 400);
        assert_eq!(check_target(dir.path()).unwrap_err().status, 400);
        assert_eq!(check_target(&dir.path().join("missing/out.html")).unwrap_err().status, 404);
    }
}
