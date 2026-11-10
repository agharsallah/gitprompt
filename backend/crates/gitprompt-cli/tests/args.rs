//! Argument parsing: the subcommands of cli.py plus `hook` and `spool drain`, and `--port` on the
//! bare command (`gitprompt --port N` still serves).

mod common;

use clap::Parser;
use gitprompt_cli::{Cli, Command, SpoolCommand, days};
use predicates::str::contains;
use pretty_assertions::assert_eq;

#[test]
fn the_bare_command_serves_and_takes_a_port() {
    let cli = Cli::try_parse_from(["gitprompt", "--port", "9000"]).expect("parses");
    assert_eq!((cli.port, cli.command.is_none()), (Some(9000), true));
}

#[test]
fn serve_takes_its_own_port() {
    let cli = Cli::try_parse_from(["gitprompt", "serve", "--port", "9001"]).expect("parses");
    assert!(matches!(cli.command, Some(Command::Serve { port: Some(9001) })), "{cli:?}");
}

#[test]
fn compact_is_a_dry_run_unless_applied() {
    let dry = Cli::try_parse_from(["gitprompt", "compact"]).expect("parses");
    assert!(matches!(dry.command, Some(Command::Compact { apply: false, events_older_than: None })), "{dry:?}");
    let applied = Cli::try_parse_from(["gitprompt", "compact", "--apply", "--events-older-than", "30"]).expect("parses");
    assert!(matches!(applied.command, Some(Command::Compact { apply: true, events_older_than: Some(30) })), "{applied:?}");
}

#[test]
fn hook_takes_the_source_and_spool_has_drain() {
    let hook = Cli::try_parse_from(["gitprompt", "hook", "claude-code"]).expect("parses");
    assert!(matches!(&hook.command, Some(Command::Hook { source }) if source == "claude-code"), "{hook:?}");
    let spool = Cli::try_parse_from(["gitprompt", "spool", "drain"]).expect("parses");
    assert!(matches!(spool.command, Some(Command::Spool { command: SpoolCommand::Drain })), "{spool:?}");
    assert!(Cli::try_parse_from(["gitprompt", "hook"]).is_err());
}

#[test]
fn days_must_be_a_whole_number_of_at_least_one() {
    assert_eq!((days("1"), days("30")), (Ok(1), Ok(30)));
    assert_eq!(days("0"), Err("must be at least 1".to_string()));
    assert!(days("-3").is_err() && days("soon").is_err());
}

#[test]
fn zero_days_is_refused_as_a_usage_error() {
    let home = common::home();
    home.cmd(&["compact", "--events-older-than", "0"]).assert().code(2).stderr(contains("must be at least 1"));
}

#[test]
fn help_lists_every_command() {
    let home = common::home();
    let assert = home.cmd(&["--help"]).assert().success();
    let out = common::stdout(assert.get_output());
    let missing: Vec<&str> = ["serve", "compact", "reindex", "check", "hook", "spool"].into_iter().filter(|c| !out.contains(c)).collect();
    assert_eq!(missing, Vec::<&str>::new());
}

#[test]
fn an_invalid_port_variable_is_an_error_naming_it() {
    let home = common::home();
    home.cmd(&["reindex"]).env("GITPROMPT_PORT", "not-a-port").assert().code(1).stderr(contains("GITPROMPT_PORT"));
}

#[test]
fn one_shot_commands_log_only_warnings_unless_asked() {
    use clap::Parser;
    use gitprompt_cli::{Cli, log_config};
    use gitprompt_core::Env;
    let quiet = Env::default();
    assert_eq!(log_config(&Cli::parse_from(["gitprompt", "check"]), &quiet).filter, "warn");
    assert_eq!(log_config(&Cli::parse_from(["gitprompt", "serve"]), &quiet).filter, gitprompt_server::logging::DEFAULT_FILTER);
    assert_eq!(log_config(&Cli::parse_from(["gitprompt"]), &quiet).filter, gitprompt_server::logging::DEFAULT_FILTER);
    let verbose = Env::from_pairs([("GITPROMPT_LOG_LEVEL", "debug")]);
    assert_ne!(log_config(&Cli::parse_from(["gitprompt", "check"]), &verbose).filter, "warn");
}
