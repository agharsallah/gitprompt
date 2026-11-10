//! The `gitprompt` command. Port of `src/gitprompt/cli.py`, plus the hook forwarder and the spool.
//!
//! ```text
//! gitprompt [--port N]                        serve (default)
//! gitprompt serve [--port N]                  run the receiver and dashboard until Ctrl-C
//! gitprompt compact [--apply] [--events-older-than DAYS]
//! gitprompt reindex                           rebuild the prompt index
//! gitprompt check                             per connector and app: the newest captured prompt
//! gitprompt doctor [--json]                   everything needed to debug an install; exit 1 on problems
//! gitprompt hook <source>                     forward one hook payload (stdin) to the daemon
//! gitprompt spool drain                       deliver payloads spooled while the daemon was down
//! ```
//!
//! Every command reads its settings from an [`Env`] (never `std::env` directly) and writes to the
//! [`Io`] it is given, so tests run the library in-process or the binary with an explicit
//! environment.

mod commands;
mod doctor;
mod http;

use std::io::{Read, Write};

use clap::{Parser, Subcommand};

use gitprompt_core::{Env, Settings};

#[derive(Debug, Parser)]
#[command(name = "gitprompt", about = "Capture your AI prompts locally.", version)]
pub struct Cli {
    /// Overrides GITPROMPT_PORT (default 7878); `gitprompt --port N` still serves.
    #[arg(long)]
    pub port: Option<u16>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the receiver and dashboard (default).
    Serve {
        /// Overrides GITPROMPT_PORT (default 7878).
        #[arg(long)]
        port: Option<u16>,
    },
    /// Delete stored events the source specs no longer keep, then VACUUM.
    Compact {
        /// Actually delete (default: dry run, show what would go).
        #[arg(long)]
        apply: bool,
        /// Also delete raw events older than DAYS days (at least 1).
        #[arg(long, value_name = "DAYS", value_parser = days)]
        events_older_than: Option<i64>,
    },
    /// Rebuild the prompt index (filters, search, statistics) from the stored prompts.
    Reindex,
    /// Per connector and app: the newest captured prompt and how it was attributed.
    Check,
    /// Everything needed to debug an install: data, index, daemon, lock, dashboard, specs,
    /// connectors, spool, preferences; then each problem with a one-line fix. Changes nothing.
    /// Exits 0 when healthy, 1 when something needs attention.
    Doctor {
        /// Print the report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Forward one hook payload read from stdin to the running daemon, for `source`. Never prints,
    /// always exits 0 (a tool must never see a failing hook); when the daemon is down or doesn't
    /// answer within 2 s, the payload is spooled and delivered later. Input that is not JSON is
    /// dropped. A payload the daemon refuses (4xx) is not spooled.
    Hook { source: String },
    /// The queue of hook payloads the forwarder could not deliver.
    Spool {
        #[command(subcommand)]
        command: SpoolCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum SpoolCommand {
    /// Deliver every spooled payload to the running daemon, oldest first.
    /// Prints `Delivered <n> spooled payloads, <m> left.`; exits 1 with (on stderr) `GitPrompt is not running
    /// on <receiver>: <m> spooled payloads left.` when the daemon can't be reached.
    Drain,
}

/// `value_parser` of `--events-older-than`: an integer ≥ 1, else "must be at least 1".
pub fn days(value: &str) -> Result<i64, String> {
    let days: i64 = value.trim().parse().map_err(|_| format!("{value:?} is not a whole number of days"))?;
    if days < 1 { Err("must be at least 1".into()) } else { Ok(days) }
}

/// Where a command reads and writes.
pub struct Io<'a> {
    pub stdin: &'a mut dyn Read,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
}

/// Run a parsed command with settings from `env`; the process exit code.
///
/// Output, as `cli.py` prints it:
/// * `compact`: one line per removal `<rows>  <MB>  <source> <event>  (<why>)`, then
///   `<rows>  <MB>  events to remove, database is <MB>`, then either
///   `Dry run: nothing changed. Run again with --apply to delete these events and VACUUM.` or
///   `Done: database is now <MB>.` (`<MB>` = `{:.1} MB` with thousands separators).
/// * `reindex`: `Indexed <n> prompts in <s> s.`
/// * `check`: per main connector `\n<name>: <layer> connected|not connected, ...[ (not detected on
///   this machine)]`, then one line per surface (`<surface name or "surface unknown">  <n> <unit>
///   ago  <project or "no folder">[ (<branch>)]  session <12 chars>  <model or "no model">  <n>
///   prompts[  k=v ...]`) or `  no prompt captured yet`, then `  not capturable: <app>: <why>` per
///   app. Without a database: `No database yet at <path>: start GitPrompt and send a prompt first.`
///   (and no database is created).
/// * errors: one line on stderr, exit 1.
pub fn run(cli: Cli, env: Env, io: &mut Io<'_>) -> u8 {
    // The forwarder must never disturb the tool: whatever fails, it exits 0 (see `Command::Hook`).
    if let Some(Command::Hook { source }) = &cli.command {
        return match Settings::from_env(env) {
            Ok(settings) => commands::hook(&settings, source, io),
            Err(_) => 0,
        };
    }
    let result = Settings::from_env(env).map_err(commands::Failure::from).and_then(|settings| match cli.command {
        None => commands::serve(settings, cli.port, io),
        Some(Command::Serve { port }) => commands::serve(settings, port.or(cli.port), io),
        Some(Command::Compact { apply, events_older_than }) => commands::compact(&settings, apply, events_older_than, io),
        Some(Command::Reindex) => commands::reindex(&settings, io),
        Some(Command::Check) => commands::check(&settings, io),
        Some(Command::Doctor { json }) => doctor::run(&settings, json, io),
        Some(Command::Spool { command: SpoolCommand::Drain }) => commands::drain(&settings, io),
        Some(Command::Hook { .. }) => Ok(0), // handled above
    });
    result.unwrap_or_else(|failure| {
        // Nothing more can be reported when stderr itself fails.
        let _ = writeln!(io.stderr, "{failure}");
        1
    })
}

/// Logging for a command: the daemon (`serve`) reports its lifecycle at info by default; one-shot
/// commands only print warnings unless `GITPROMPT_LOG_LEVEL` asks for more, so their stderr stays
/// as quiet as their output is meant to be read.
pub fn log_config(cli: &Cli, env: &Env) -> gitprompt_server::logging::LogConfig {
    let mut config = gitprompt_server::logging::LogConfig::from_env(env);
    let serving = matches!(cli.command, None | Some(Command::Serve { .. }));
    if !serving && env.get(gitprompt_server::logging::LEVEL_ENV).is_none() {
        config.filter = "warn".into();
    }
    config
}

/// The binary's entry point: parse `std::env::args`, take [`Env::process`], real stdio. Logs go to
/// stderr ([`gitprompt_server::logging`]), except for the hook forwarder, which never logs.
pub fn main_entry() -> std::process::ExitCode {
    let cli = Cli::parse();
    let env = Env::process();
    if !matches!(cli.command, Some(Command::Hook { .. })) {
        // Stderr only: there is no file to fail to open.
        let _ = gitprompt_server::logging::init(&log_config(&cli, &env));
    }
    let (mut stdin, mut stdout, mut stderr) = (std::io::stdin(), std::io::stdout(), std::io::stderr());
    let mut io = Io { stdin: &mut stdin, stdout: &mut stdout, stderr: &mut stderr };
    std::process::ExitCode::from(run(cli, env, &mut io))
}
