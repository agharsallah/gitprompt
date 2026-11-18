//! The `gitprompt` command. Port of `src/gitprompt/cli.py`, plus the hook forwarder and the spool.
//!
//! ```text
//! gitprompt [--port N]                        serve (default)
//! gitprompt serve [--port N]                  run the receiver and dashboard until Ctrl-C
//! gitprompt compact [--apply] [--events-older-than DAYS]
//! gitprompt reindex                           rebuild the prompt index
//! gitprompt check                             per connector and app: the newest captured prompt
//! gitprompt doctor [--json]                   everything needed to debug an install; exit 1 on problems
//! gitprompt find [words] [--project P] [--limit N] [--print N] [--json]
//!                                             the prompts matching the words (none: the latest)
//! gitprompt resume <session id or name> | --last [--json]
//!                                             print the command that resumes a session
//! gitprompt why <commit> [--json]             the prompt behind a commit (a hash, HEAD, HEAD~2...)
//! gitprompt why --branch <name> [--project P] the prompts behind a branch, as Markdown
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
mod lookup;
mod print;

use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use jiff::Timestamp;

use gitprompt_core::{Env, Settings};

#[derive(Debug, Parser)]
#[command(name = "gitprompt", about = "Capture your AI prompts locally.", version)]
pub struct Cli {
    /// Overrides `GITPROMPT_PORT` (default 7878); `gitprompt --port N` still serves.
    #[arg(long)]
    pub port: Option<u16>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the receiver and dashboard (default).
    Serve {
        /// Overrides `GITPROMPT_PORT` (default 7878).
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
    /// Find a past prompt: the prompts whose text has all the words, best first (the same search
    /// as the dashboard's ⌘K). Without words, the latest prompts. Exits 0 when something matched,
    /// 1 when nothing did.
    #[command(after_help = FIND_EXAMPLES)]
    Find(FindArgs),
    /// Print the command that resumes a session (`cd <folder> && <command>`); it is never run.
    /// Exits 0 when a command was printed, 1 when there is none: no such session, a name that
    /// matches several sessions, or a tool that cannot resume from the command line.
    #[command(after_help = RESUME_EXAMPLES)]
    Resume(ResumeArgs),
    /// What made the code. `why <commit>`: which prompt made a commit, and in which session; the
    /// first line is the verdict (traced, likely or made by hand), then the prompt's first lines
    /// and its place in the dashboard. A revision (HEAD, HEAD~2, a branch) resolves with git in
    /// the current folder; a hash works anywhere. Exits 0 when the commit was found, 1 when not,
    /// or when a prefix names several. `why --branch <name>`: the prompts behind a branch as
    /// Markdown to paste into a pull request: each prompt you typed on it, folded, with the
    /// commits its work made (the block the dashboard's Commits tab copies; secrets masked, paths
    /// relative). Exits 0 when the block was printed, 1 when no prompt you typed was recorded on
    /// that branch.
    #[command(after_help = WHY_EXAMPLES)]
    Why(WhyArgs),
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

const FIND_EXAMPLES: &str = "\
Examples:
  gitprompt find auth                    the latest prompts about auth, numbered
  gitprompt find auth --print 2 | pbcopy copy the full text of match 2 (xclip, wl-copy or clip.exe elsewhere)
  gitprompt find --project api --json    the latest prompts of the project api, for scripts";

const RESUME_EXAMPLES: &str = "\
Examples:
  gitprompt resume --last | pbcopy       copy the command that resumes the latest session
  gitprompt resume \"release prep\"        the session you named so (or its id)
  gitprompt resume 7f3a9c1e --json       the command, folder and tool as JSON";

const WHY_EXAMPLES: &str = "\
Examples:
  gitprompt why HEAD                                the prompt behind the last commit
  gitprompt why a1b2c3d --json                      the commit, its trace and its project as JSON
  gitprompt why --branch timing-ledger | pbcopy     copy the prompts behind a branch for its pull request
  gitprompt why --branch main --project api         the branch of one project (its folder name, path or name)
  git config alias.why '!gitprompt why'             then `git why HEAD~2` works (--global: in every repository)";

#[derive(Debug, Args)]
pub struct FindArgs {
    /// Words to look for in what you typed (all of them, any order, ignoring case).
    pub words: Vec<String>,
    /// Only prompts of this project: its folder name, its path or its name.
    #[arg(long, value_name = "PROJECT")]
    pub project: Option<String>,
    /// How many matches to list (1 to 100).
    #[arg(long, value_name = "N", default_value = "10", value_parser = limit)]
    pub limit: usize,
    /// Print the full text of match N, and nothing else, ready to pipe to the clipboard.
    #[arg(long, value_name = "N", value_parser = limit, conflicts_with = "json")]
    pub print: Option<usize>,
    /// Print the matches as JSON (an empty list, and exit 1, when none match).
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ResumeArgs {
    /// The session's id, or the name you gave it (or its label).
    #[arg(required_unless_present = "last", conflicts_with = "last")]
    pub session: Option<String>,
    /// The most recent session.
    #[arg(long)]
    pub last: bool,
    /// Print the command, folder and tool as JSON.
    #[arg(long)]
    pub json: bool,
}

/// What `gitprompt why` explains: a commit, or a branch. Each question is one more argument here
/// and one more module in `lookup::why`.
#[derive(Debug, Args)]
pub struct WhyArgs {
    /// The commit: a hash or a prefix of one (at least 4 hex digits), or a revision git resolves
    /// here (HEAD, HEAD~2, a branch).
    pub commit: Option<String>,
    /// Print the commit's lookup as JSON: the commit, its trace and its project, or the candidates.
    #[arg(long, conflicts_with = "branch")]
    pub json: bool,
    /// The branch whose prompts to print (as recorded when you typed them), instead of a commit.
    #[arg(long, value_name = "BRANCH", required_unless_present = "commit", conflicts_with = "commit")]
    pub branch: Option<String>,
    /// The project the branch is in: its folder name, its path or its name. Default: the project
    /// where you last typed a prompt on that branch.
    #[arg(long, value_name = "PROJECT", conflicts_with = "commit")]
    pub project: Option<String>,
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

/// `value_parser` of `--limit` and `--print`: a whole number from 1 to 100.
pub fn limit(value: &str) -> Result<usize, String> {
    let n: usize = value.trim().parse().map_err(|_| format!("{value:?} is not a whole number"))?;
    if (1..=100).contains(&n) { Ok(n) } else { Err("must be from 1 to 100".into()) }
}

/// Where a command reads and writes, and what it knows of the outside: the time it runs at (the
/// entry point reads the clock once, so printing is deterministic in tests), the folder it runs in
/// (where `why` resolves a revision) and whether stdout is a terminal (colour and a final newline
/// only then).
pub struct Io<'a> {
    pub stdin: &'a mut dyn Read,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
    pub now: Timestamp,
    pub cwd: PathBuf,
    pub tty: bool,
}

impl std::fmt::Debug for Io<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Io").finish_non_exhaustive()
    }
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
pub fn run(cli: &Cli, env: Env, io: &mut Io<'_>) -> u8 {
    // The forwarder must never disturb the tool: whatever fails, it exits 0 (see `Command::Hook`).
    if let Some(Command::Hook { source }) = &cli.command {
        return match Settings::from_env(env) {
            Ok(settings) => commands::hook(&settings, source, io),
            Err(_) => 0,
        };
    }
    let result = Settings::from_env(env).map_err(commands::Failure::from).and_then(|settings| match &cli.command {
        None => commands::serve(settings, cli.port, io),
        Some(Command::Serve { port }) => commands::serve(settings, port.or(cli.port), io),
        Some(Command::Compact { apply, events_older_than }) => commands::compact(&settings, *apply, *events_older_than, io),
        Some(Command::Reindex) => commands::reindex(&settings, io),
        Some(Command::Find(args)) => lookup::find(&settings, args, io),
        Some(Command::Resume(args)) => lookup::resume(&settings, args, io),
        Some(Command::Why(args)) => lookup::why(&settings, args, io),
        Some(Command::Check) => commands::check(&settings, io),
        Some(Command::Doctor { json }) => doctor::run(&settings, *json, io),
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
    let tty = stdout.is_terminal();
    // A folder that is gone (deleted under the shell) resolves no revision; a hash still works.
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut io = Io { stdin: &mut stdin, stdout: &mut stdout, stderr: &mut stderr, now: Timestamp::now(), cwd, tty };
    std::process::ExitCode::from(run(&cli, env, &mut io))
}
