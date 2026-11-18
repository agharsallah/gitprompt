//! The library's `run` in-process, with an `Io` of buffers: a fixed time, a chosen folder, a
//! terminal or a pipe. What a command prints is then exact. Data is put in place through the
//! server's Service, as a daemon would have stored it.

use std::path::PathBuf;

use clap::Parser;
use jiff::Timestamp;

use gitprompt_cli::{Cli, Io, run};
use gitprompt_core::Env;
use gitprompt_server::{App, AppOptions};
use serde_json::Value;

use super::Home;

pub struct Ran {
    pub code: u8,
    pub out: String,
    pub err: String,
}

/// Where and how a command runs.
pub struct Call {
    pub now: Timestamp,
    pub cwd: PathBuf,
    pub tty: bool,
    pub no_color: bool,
}

impl Call {
    /// Piped, in `/` (no repository), colour allowed.
    pub fn at(now: Timestamp) -> Self {
        Call { now, cwd: PathBuf::from("/"), tty: false, no_color: false }
    }
}

/// `gitprompt <args>` over `home`, as `call` says.
pub fn run_cli(home: &Home, args: &[&str], call: Call) -> Ran {
    let cli = Cli::try_parse_from(std::iter::once("gitprompt").chain(args.iter().copied())).expect("parses");
    let mut env = Env::from_pairs([
        ("HOME", home.settings.user_home.display().to_string()),
        ("GITPROMPT_HOME", home.settings.home.display().to_string()),
    ]);
    if call.no_color {
        env.set("NO_COLOR", "1");
    }
    let (mut stdin, mut out, mut err) = (std::io::empty(), Vec::new(), Vec::new());
    let mut io = Io { stdin: &mut stdin, stdout: &mut out, stderr: &mut err, now: call.now, cwd: call.cwd, tty: call.tty };
    let code = run(&cli, env, &mut io);
    Ran { code, out: String::from_utf8(out).expect("utf-8"), err: String::from_utf8(err).expect("utf-8") }
}

/// Ingest each payload into `home` through a Service with `options` (its clock is when they
/// arrive), then call each `(session, name)` so.
pub fn seed_with(home: &Home, options: AppOptions, payloads: Vec<(&str, Value)>, names: &[(&str, &str)]) {
    let service = App::new(home.settings.clone(), options).expect("app").service();
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
    runtime.block_on(async {
        for (source, payload) in payloads {
            service.ingest(source, payload).await.expect("ingest");
        }
        for (session, name) in names {
            service.set_name("session", session, name).await.expect("name");
        }
    });
}
