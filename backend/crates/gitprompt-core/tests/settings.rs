//! Settings: env-driven paths and port, the hook token (created once, rotatable).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, fixtures use small known values, and helpers are crate-private anyway"
)]

mod common;

use std::path::PathBuf;

use common::{mode, sandbox};
use gitprompt_core::settings::{DEFAULT_PORT, HOST};
use gitprompt_core::{Env, Error, Settings};
use pretty_assertions::assert_eq;

fn from_env(pairs: &[(&str, &str)]) -> gitprompt_core::Result<Settings> {
    Settings::from_env(Env::from_pairs(pairs.iter().copied()))
}

// --- from_env -------------------------------------------------------------------------------

#[test]
fn defaults_put_data_in_dot_gitprompt_under_home() {
    let settings = from_env(&[("HOME", "/users/ann")]).unwrap();
    assert_eq!(settings.home, PathBuf::from("/users/ann/.gitprompt"));
    assert_eq!(settings.user_home, PathBuf::from("/users/ann"));
    assert_eq!((settings.port, settings.log_level.as_str()), (7878, "warning"));
}

#[test]
fn gitprompt_home_is_used_as_given() {
    let settings = from_env(&[("HOME", "/users/ann"), ("GITPROMPT_HOME", "/data/gp")]).unwrap();
    assert_eq!(settings.home, PathBuf::from("/data/gp"));
}

#[test]
fn a_tilde_in_gitprompt_home_is_expanded_against_home() {
    let settings = from_env(&[("HOME", "/users/ann"), ("GITPROMPT_HOME", "~/gp-data")]).unwrap();
    assert_eq!(settings.home, PathBuf::from("/users/ann/gp-data"));
}

#[test]
fn port_and_log_level_come_from_the_environment() {
    let settings = from_env(&[("HOME", "/h"), ("GITPROMPT_PORT", "9000"), ("GITPROMPT_LOG_LEVEL", "debug")]).unwrap();
    assert_eq!((settings.port, settings.log_level.as_str()), (9000, "debug"));
}

#[test]
fn an_empty_port_variable_means_the_default_port() {
    assert_eq!(from_env(&[("HOME", "/h"), ("GITPROMPT_PORT", "")]).unwrap().port, DEFAULT_PORT);
}

#[test]
fn the_highest_port_is_accepted() {
    assert_eq!(from_env(&[("HOME", "/h"), ("GITPROMPT_PORT", "65535")]).unwrap().port, 65535);
}

#[test]
fn a_port_that_is_not_a_number_is_invalid_and_names_the_variable() {
    let err = from_env(&[("HOME", "/h"), ("GITPROMPT_PORT", "eighty")]).unwrap_err();
    assert!(matches!(&err, Error::Invalid(m) if m.contains("GITPROMPT_PORT")), "{err:?}");
}

#[test]
fn a_port_above_65535_is_invalid() {
    let err = from_env(&[("HOME", "/h"), ("GITPROMPT_PORT", "65536")]).unwrap_err();
    assert!(matches!(&err, Error::Invalid(m) if m.contains("GITPROMPT_PORT")), "{err:?}");
}

#[test]
fn the_environment_is_kept_for_specs_and_preferences() {
    let env = Env::from_pairs([("HOME", "/h"), ("CLAUDE_CONFIG_DIR", "/elsewhere")]);
    assert_eq!(Settings::from_env(env.clone()).unwrap().env, env);
}

// --- derived values -------------------------------------------------------------------------

#[test]
fn the_receiver_is_loopback_on_the_configured_port() {
    let mut settings = Settings::new("/gp", "/home");
    assert_eq!((settings.host(), HOST), ("127.0.0.1", "127.0.0.1"));
    assert_eq!(settings.receiver(), "http://127.0.0.1:7878");
    settings.port = 9001;
    assert_eq!(settings.receiver(), "http://127.0.0.1:9001");
}

#[test]
fn data_files_live_under_home() {
    let settings = Settings::new("/gp", "/home");
    let paths = [settings.db_path(), settings.user_sources(), settings.backups(), settings.token_file()];
    let expected = ["/gp/gitprompt.db", "/gp/sources", "/gp/backups", "/gp/token"].map(PathBuf::from);
    assert_eq!(paths, expected);
}

#[test]
fn spool_dir_and_lock_file_are_their_own_entries_under_home() {
    let settings = Settings::new("/gp", "/home");
    let (spool, lock) = (settings.spool_dir(), settings.lock_file());
    assert_eq!((spool.parent(), lock.parent()), (Some(PathBuf::from("/gp").as_path()), Some(PathBuf::from("/gp").as_path())));
    let others = [settings.db_path(), settings.user_sources(), settings.backups(), settings.token_file()];
    assert!(spool != lock && !others.contains(&spool) && !others.contains(&lock), "{spool:?} {lock:?}");
}

// --- token ----------------------------------------------------------------------------------

#[test]
fn the_token_is_created_once_url_safe_and_long() {
    let sb = sandbox();
    let token = sb.settings.token().unwrap();
    assert!(token.len() >= 43, "32 random bytes give at least 43 URL-safe characters: {token}");
    assert!(token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'), "{token}");
    assert_eq!(sb.settings.token().unwrap(), token);
}

#[test]
fn the_token_is_readable_by_the_user_only() {
    let sb = sandbox();
    sb.settings.token().unwrap();
    assert_eq!((mode(&sb.settings.home), mode(&sb.settings.token_file())), (0o700, 0o600));
}

#[test]
fn the_token_is_stored_in_the_token_file() {
    let sb = sandbox();
    let token = sb.settings.token().unwrap();
    assert_eq!(std::fs::read_to_string(sb.settings.token_file()).unwrap().trim(), token);
}

#[test]
fn an_existing_token_file_is_used_trimmed() {
    let sb = sandbox();
    std::fs::create_dir_all(&sb.settings.home).unwrap();
    std::fs::write(sb.settings.token_file(), "existing-token\n").unwrap();
    assert_eq!(sb.settings.token().unwrap(), "existing-token");
}

#[test]
fn two_data_directories_get_different_tokens() {
    let (a, b) = (sandbox(), sandbox());
    assert_ne!(a.settings.token().unwrap(), b.settings.token().unwrap());
}

#[test]
fn rotating_replaces_the_token_with_a_new_one() {
    let sb = sandbox();
    let old = sb.settings.token().unwrap();
    let new = sb.settings.rotate_token().unwrap();
    assert_ne!(new, old);
    assert_eq!(sb.settings.token().unwrap(), new);
    assert_eq!(mode(&sb.settings.token_file()), 0o600);
}

#[test]
fn rotating_before_any_token_exists_creates_one() {
    let sb = sandbox();
    let new = sb.settings.rotate_token().unwrap();
    assert!(new.len() >= 43, "{new}");
    assert_eq!(sb.settings.token().unwrap(), new);
}
