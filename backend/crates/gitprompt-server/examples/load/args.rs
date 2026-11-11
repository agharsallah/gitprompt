//! Command-line arguments.

use std::path::PathBuf;
use std::time::Duration;

pub(crate) const USAGE: &str = "usage: load [--prompts N] [--events-per-prompt M] [--runs R] [--home DIR] [--json]
            [--concurrent S] [--git-ms MS] [--check] [--max-read-p95-ms MS] [--max-rss-mb MB]

  --prompts N            turns to ingest (default 2000)
  --events-per-prompt M  PostToolUse events per turn (default 20)
  --runs R               timed runs per read, after one cold run (default 20)
  --home DIR             data directory to fill (default: a fresh temp directory)
  --json                 print machine-readable results instead of a table
  --concurrent S         then S seconds of ingest and dashboard reads at once (default 0: off)
  --git-ms MS            how long git answers during the concurrent phase (default 600)
  --check                fail (exit 1) when a budget is exceeded; implies --concurrent 10
  --max-read-p95-ms MS   budget: p95 of every read, alone and under load (default 300)
  --max-rss-mb MB        budget: peak resident memory (default 1024)";

pub(crate) struct Args {
    pub(crate) prompts: usize,
    pub(crate) events_per_prompt: usize,
    pub(crate) runs: usize,
    pub(crate) home: Option<PathBuf>,
    pub(crate) json: bool,
    pub(crate) concurrent: Duration,
    pub(crate) git_ms: u64,
    pub(crate) check: bool,
    pub(crate) max_read_p95_ms: f64,
    pub(crate) max_rss_mb: u64,
}

pub(crate) fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        prompts: 2000,
        events_per_prompt: 20,
        runs: 20,
        home: None,
        json: false,
        concurrent: Duration::ZERO,
        git_ms: 600,
        check: false,
        max_read_p95_ms: 300.0,
        max_rss_mb: 1024,
    };
    let mut argv = std::env::args().skip(1);
    while let Some(flag) = argv.next() {
        let mut value = |name: &str| argv.next().ok_or_else(|| format!("{name} needs a value"));
        let mut count = |name: &str| -> Result<usize, String> {
            let text = value(name)?;
            text.parse().map_err(|_| format!("{name}: {text:?} is not a whole number"))
        };
        match flag.as_str() {
            "--prompts" => args.prompts = count("--prompts")?,
            "--events-per-prompt" => args.events_per_prompt = count("--events-per-prompt")?,
            "--runs" => args.runs = count("--runs")?.max(1),
            "--home" => args.home = Some(PathBuf::from(value("--home")?)),
            "--json" => args.json = true,
            "--concurrent" => args.concurrent = Duration::from_secs(count("--concurrent")? as u64),
            "--git-ms" => args.git_ms = count("--git-ms")? as u64,
            "--check" => args.check = true,
            "--max-read-p95-ms" => args.max_read_p95_ms = count("--max-read-p95-ms")? as f64,
            "--max-rss-mb" => args.max_rss_mb = count("--max-rss-mb")? as u64,
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    if args.prompts == 0 {
        return Err("--prompts must be at least 1".into());
    }
    if args.check && args.concurrent.is_zero() {
        args.concurrent = Duration::from_secs(10);
    }
    Ok(args)
}
