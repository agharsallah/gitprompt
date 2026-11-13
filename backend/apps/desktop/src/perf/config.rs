//! What a performance run does, from the environment (`.env.performance` and the variables
//! `just quality desktop` sets per launch):
//!
//! | variable                      | meaning                                                      | default |
//! |-------------------------------|--------------------------------------------------------------|---------|
//! | `GITPROMPT_PERF_OUT`          | the JSON file the results are written to (required)          |         |
//! | `GITPROMPT_PERF_ROUNDS`       | rounds through the sidebar's pages                           | `3`     |
//! | `GITPROMPT_PERF_IDLE_SECONDS` | how long the app stays open, untouched, after writing        | `0`     |
//! | `GITPROMPT_PERF_COLD`         | `true`: a private webview store (empty cache), a cold launch | `false` |

use std::path::PathBuf;
use std::time::Duration;

use gitprompt_core::Env;

const OUT_ENV: &str = "GITPROMPT_PERF_OUT";
const ROUNDS_ENV: &str = "GITPROMPT_PERF_ROUNDS";
const IDLE_ENV: &str = "GITPROMPT_PERF_IDLE_SECONDS";
const COLD_ENV: &str = "GITPROMPT_PERF_COLD";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PerfConfig {
    pub(super) out: PathBuf,
    pub(super) rounds: usize,
    /// Untouched time after the results are written, for whoever measures the idle app.
    pub(super) idle: Duration,
    /// A private webview store: nothing cached from an earlier launch.
    pub(super) cold: bool,
}

impl PerfConfig {
    pub(super) fn from_env(env: &Env) -> Result<Self, String> {
        let out = env.get(OUT_ENV).ok_or_else(|| format!("the performance profile needs {OUT_ENV}: the file to write the results to"))?;
        Ok(PerfConfig {
            out: PathBuf::from(out),
            rounds: number(env, ROUNDS_ENV, 3)?,
            idle: Duration::from_secs(number(env, IDLE_ENV, 0)?),
            cold: matches!(env.get(COLD_ENV), Some("1" | "true" | "yes")),
        })
    }
}

fn number<T: std::str::FromStr>(env: &Env, name: &str, default: T) -> Result<T, String> {
    match env.get(name) {
        None => Ok(default),
        Some(text) => text.trim().parse().map_err(|_| format!("{name}={text}: expected a whole number")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_an_output_file() {
        let error = PerfConfig::from_env(&Env::default()).unwrap_err();
        assert!(error.contains(OUT_ENV), "{error}");
    }

    #[test]
    fn defaults_then_overrides() {
        let config = PerfConfig::from_env(&Env::from_pairs([(OUT_ENV, "/tmp/perf.json")])).unwrap();
        assert_eq!(config, PerfConfig { out: "/tmp/perf.json".into(), rounds: 3, idle: Duration::ZERO, cold: false });
        let env = Env::from_pairs([(OUT_ENV, "out.json"), (ROUNDS_ENV, "5"), (IDLE_ENV, "15"), (COLD_ENV, "true")]);
        let config = PerfConfig::from_env(&env).unwrap();
        assert_eq!((config.rounds, config.idle, config.cold), (5, Duration::from_secs(15), true));
    }

    #[test]
    fn a_bad_number_names_its_variable() {
        let error = PerfConfig::from_env(&Env::from_pairs([(OUT_ENV, "o.json"), (ROUNDS_ENV, "many")])).unwrap_err();
        assert!(error.contains("GITPROMPT_PERF_ROUNDS=many"), "{error}");
    }
}
