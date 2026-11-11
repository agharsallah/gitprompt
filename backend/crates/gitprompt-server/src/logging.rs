//! Logging for the binaries (`gitprompt` and the desktop app): one [`init`], configured from the
//! environment by [`LogConfig::from_env`].
//!
//! | variable              | meaning                                                         | default |
//! |-----------------------|-----------------------------------------------------------------|---------|
//! | `GITPROMPT_LOG_LEVEL` | a level for GitPrompt's crates (`debug`), or a full filter (`gitprompt=debug,tower=warn`) | `info` for GitPrompt, `warn` for the rest |
//! | `GITPROMPT_LOG_FORMAT`| `json` for one JSON object per line, else human-readable text   | text    |
//!
//! A level applies to every `gitprompt_*` crate; other crates log at `warn` (or that level when it
//! is quieter). Python's names `warning`, `critical` and `fatal` are accepted. Logs go to stderr
//! and, when [`LogConfig::file`] is set, to that file too (appended; the folder is created 0700 and
//! the file 0600, since request paths name sessions and projects). The hook forwarder never logs:
//! a tool shows anything on stderr as a hook error.

use std::fs::{File, OpenOptions};
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tracing::Subscriber;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use gitprompt_core::Env;

/// The level or filter variable.
pub const LEVEL_ENV: &str = "GITPROMPT_LOG_LEVEL";
/// The format variable (`json` or `text`).
pub const FORMAT_ENV: &str = "GITPROMPT_LOG_FORMAT";
/// `info` for GitPrompt's crates (every target starting with `gitprompt`), `warn` for the rest.
pub const DEFAULT_FILTER: &str = "warn,gitprompt=info";
/// A log file larger than this at start is moved to `<name>.1` (the previous `.1` is replaced).
pub const ROTATE_BYTES: u64 = 10 * 1024 * 1024;

/// How each line is written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogFormat {
    /// `2026-11-10T10:00:00.000000Z  INFO gitprompt_server::daemon: GitPrompt is running addr=...`
    #[default]
    Text,
    /// One JSON object per line (`timestamp`, `level`, `target`, `fields`...).
    Json,
}

/// What [`init`] installs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogConfig {
    /// An `EnvFilter` directive string; see [`filter`].
    pub filter: String,
    pub format: LogFormat,
    /// Write to stderr.
    pub stderr: bool,
    /// Also append to this file.
    pub file: Option<PathBuf>,
}

impl Default for LogConfig {
    /// [`DEFAULT_FILTER`], text, stderr only.
    fn default() -> Self {
        LogConfig { filter: DEFAULT_FILTER.to_owned(), format: LogFormat::Text, stderr: true, file: None }
    }
}

impl LogConfig {
    /// Filter from [`LEVEL_ENV`], format from [`FORMAT_ENV`], stderr only.
    pub fn from_env(env: &Env) -> Self {
        let format = match env.get(FORMAT_ENV).map(str::trim) {
            Some(format) if format.eq_ignore_ascii_case("json") => LogFormat::Json,
            _ => LogFormat::Text,
        };
        LogConfig { filter: filter(env.get(LEVEL_ENV)), format, ..LogConfig::default() }
    }

    /// The same configuration, also appending to `file`.
    #[must_use]
    pub fn with_file(self, file: impl Into<PathBuf>) -> Self {
        LogConfig { file: Some(file.into()), ..self }
    }
}

/// The desktop app's log file: `<data dir>/logs/desktop.log`.
pub fn desktop_log_file(home: &Path) -> PathBuf {
    home.join("logs").join("desktop.log")
}

/// Levels from quietest to most verbose.
const LEVELS: [&str; 6] = ["off", "error", "warn", "info", "debug", "trace"];

/// The filter for a [`LEVEL_ENV`] value: unset or blank = [`DEFAULT_FILTER`]; a level (any case,
/// Python names too) = `<others>,gitprompt=<level>` where others log at `warn` or that level
/// when it is quieter; anything else is taken as a full `EnvFilter` directive string.
pub fn filter(value: Option<&str>) -> String {
    let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) else { return DEFAULT_FILTER.to_owned() };
    let level = match value.to_ascii_lowercase().as_str() {
        "warning" => "warn".to_owned(),
        "critical" | "fatal" => "error".to_owned(),
        other => other.to_owned(),
    };
    let Some(rank) = LEVELS.iter().position(|l| *l == level) else { return value.to_owned() };
    let others = LEVELS.get(rank.min(2)).unwrap_or(&"warn"); // 2 = warn
    format!("{others},gitprompt={level}")
}

/// Install the global subscriber described by `config`, once per process: a later call (or a
/// subscriber installed by someone else) leaves the first one in place and returns `Ok`.
///
/// An invalid filter falls back to [`DEFAULT_FILTER`] (with a warning logged). When the log file
/// can't be opened, logging still goes to stderr and the error is returned for the caller to report.
pub fn init(config: &LogConfig) -> io::Result<()> {
    let (filter, bad_filter) = match EnvFilter::try_new(&config.filter) {
        Ok(filter) => (filter, None),
        Err(e) => (EnvFilter::new(DEFAULT_FILTER), Some(e.to_string())),
    };
    let (file, file_error) = match config.file.as_deref().map(open_log_file).transpose() {
        Ok(file) => (file, None),
        Err(e) => (None, Some(e)),
    };
    let mut layers = Vec::new();
    if config.stderr {
        layers.push(layer(config.format, io::stderr, io::stderr().is_terminal()));
    }
    if let Some(file) = file {
        layers.push(layer(config.format, Arc::new(file), false));
    }
    if tracing_subscriber::registry().with(filter).with(layers).try_init().is_err() {
        return Ok(()); // already installed: the first configuration stays
    }
    if let Some(error) = bad_filter {
        tracing::warn!(filter = %config.filter, %error, "{LEVEL_ENV} is not a valid filter; using {DEFAULT_FILTER}");
    }
    match file_error {
        Some(error) => {
            let file = config.file.as_deref().map(Path::display);
            tracing::warn!(file = ?file, %error, "cannot open the log file; logging to stderr only");
            Err(error)
        }
        None => Ok(()),
    }
}

/// One formatting layer writing to `writer`.
fn layer<S, W>(format: LogFormat, writer: W, ansi: bool) -> Box<dyn Layer<S> + Send + Sync>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'w> MakeWriter<'w> + Send + Sync + 'static,
{
    let base = tracing_subscriber::fmt::layer().with_writer(writer).with_ansi(ansi);
    match format {
        LogFormat::Text => base.boxed(),
        LogFormat::Json => base.json().flatten_event(true).boxed(),
    }
}

/// Open `path` for appending, creating its folder (0700) and the file (0600); a file over
/// [`ROTATE_BYTES`] is moved aside first.
fn open_log_file(path: &Path) -> io::Result<File> {
    if let Some(dir) = path.parent() {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(dir)?;
    }
    if std::fs::metadata(path).is_ok_and(|meta| meta.len() > ROTATE_BYTES) {
        let mut rotated = path.as_os_str().to_owned();
        rotated.push(".1");
        std::fs::rename(path, rotated)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let file = options.open(path)?;
    #[cfg(unix)]
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn unset_or_blank_is_the_default_filter() {
        assert_eq!(filter(None), DEFAULT_FILTER);
        assert_eq!(filter(Some("  ")), DEFAULT_FILTER);
    }

    #[test]
    fn a_level_applies_to_gitprompt_and_caps_the_rest_at_warn() {
        assert_eq!(filter(Some("debug")), "warn,gitprompt=debug");
        assert_eq!(filter(Some("TRACE")), "warn,gitprompt=trace");
        assert_eq!(filter(Some("info")), "warn,gitprompt=info");
        assert_eq!(filter(Some("warning")), "warn,gitprompt=warn");
        assert_eq!(filter(Some("error")), "error,gitprompt=error");
        assert_eq!(filter(Some("critical")), "error,gitprompt=error");
        assert_eq!(filter(Some("off")), "off,gitprompt=off");
    }

    #[test]
    fn anything_else_is_a_full_filter() {
        assert_eq!(filter(Some("gitprompt=debug,tower=warn")), "gitprompt=debug,tower=warn");
        assert_eq!(filter(Some(" hyper=info ")), "hyper=info");
    }

    #[test]
    fn every_filter_built_from_a_level_is_valid() {
        for level in LEVELS {
            assert!(EnvFilter::try_new(filter(Some(level))).is_ok(), "{level}");
        }
        assert!(EnvFilter::try_new(DEFAULT_FILTER).is_ok());
    }

    #[test]
    fn the_environment_picks_the_filter_and_format() {
        let env = Env::from_pairs([(LEVEL_ENV, "debug"), (FORMAT_ENV, "JSON")]);
        let config = LogConfig::from_env(&env);
        assert_eq!(config, LogConfig { filter: "warn,gitprompt=debug".into(), format: LogFormat::Json, stderr: true, file: None });
        assert_eq!(LogConfig::from_env(&Env::default()), LogConfig::default());
        assert_eq!(LogConfig::from_env(&Env::from_pairs([(FORMAT_ENV, "yaml")])).format, LogFormat::Text);
    }

    #[test]
    fn the_desktop_logs_under_the_data_dir() {
        assert_eq!(desktop_log_file(Path::new("/d")), PathBuf::from("/d/logs/desktop.log"));
    }

    #[test]
    fn the_log_file_is_private_and_appended() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("logs").join("desktop.log");
        std::io::Write::write_all(&mut open_log_file(&path).unwrap(), b"one\n").unwrap();
        std::io::Write::write_all(&mut open_log_file(&path).unwrap(), b"two\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\ntwo\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&path), 0o600);
            assert_eq!(mode(path.parent().unwrap()), 0o700);
        }
    }

    #[test]
    fn a_large_log_file_is_moved_aside() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("desktop.log");
        let file = File::create(&path).unwrap();
        file.set_len(ROTATE_BYTES + 1).unwrap();
        drop(open_log_file(&path).unwrap());
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);
        assert_eq!(std::fs::metadata(tmp.path().join("desktop.log.1")).unwrap().len(), ROTATE_BYTES + 1);
    }
}
