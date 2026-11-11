//! Loading specs from the bundle and `<home>/sources/`, one file at a time, and the `SpecCache`
//! (port of `tests/sources/test_spec.py` loading tests, `tests/test_extension_points.py` and the
//! spec-problem parts of `tests/api/test_connectors.py`).
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

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use common::{MINIMAL, Sandbox, bundled, sandbox};
use gitprompt_core::Error;
use gitprompt_core::spec::{SpecCache, SpecFile, SpecOrigin, SpecProblem, SpecSet, load_spec_set, load_specs, read_spec_file};
use pretty_assertions::assert_eq;

const BUNDLED_IDS: [&str; 5] = ["claude-code", "claude-code-otel", "codex", "copilot-cli", "cursor"];

fn ids(set: &SpecSet) -> Vec<&str> {
    set.specs.keys().map(String::as_str).collect()
}

fn problem_names(set: &SpecSet) -> Vec<&str> {
    set.problems.iter().map(|p| p.file.name.as_str()).collect()
}

fn user_file(sb: &Sandbox, name: &str, overrides: bool) -> SpecFile {
    let path = sb.settings.user_sources().join(name).display().to_string();
    SpecFile { name: name.into(), path, origin: SpecOrigin::User, overrides }
}

// --- the bundle alone -----------------------------------------------------------------------

#[test]
fn without_user_specs_every_bundled_spec_loads_without_problems() {
    let sb = sandbox();
    let set = load_spec_set(&sb.settings);
    assert_eq!(ids(&set), BUNDLED_IDS);
    assert_eq!(set.problems, Vec::<SpecProblem>::new());
}

#[test]
fn bundled_files_are_named_with_a_bundled_path() {
    let sb = sandbox();
    let file = load_spec_set(&sb.settings).files["claude-code-otel"].clone();
    let expected = SpecFile {
        name: "claude-code-otel.yaml".into(),
        path: "bundled:claude-code-otel.yaml".into(),
        origin: SpecOrigin::Bundled,
        overrides: false,
    };
    assert_eq!(file, expected);
}

#[test]
fn load_specs_is_the_usable_specs_of_the_set() {
    let sb = sandbox();
    sb.user_spec("broken.yaml", "id: [unclosed\n");
    sb.user_spec("mytool.yaml", MINIMAL);
    let specs = load_specs(&sb.settings);
    assert_eq!(
        specs.keys().map(String::as_str).collect::<Vec<_>>(),
        ["claude-code", "claude-code-otel", "codex", "copilot-cli", "cursor", "mytool"]
    );
}

#[test]
fn spec_set_get_finds_a_spec_by_id() {
    let sb = sandbox();
    let set = load_spec_set(&sb.settings);
    assert_eq!(set.get("codex").map(|s| s.name.as_str()), Some("OpenAI Codex CLI"));
    assert!(set.get("nope").is_none());
}

// --- user specs -----------------------------------------------------------------------------

#[test]
fn a_user_spec_adds_a_source() {
    let sb = sandbox();
    sb.user_spec("mytool.yaml", MINIMAL);
    let set = load_spec_set(&sb.settings);
    assert_eq!(set.specs["mytool"].name, "My tool");
    assert_eq!(set.files["mytool"], user_file(&sb, "mytool.yaml", false));
    assert_eq!(set.problems, Vec::<SpecProblem>::new());
}

#[test]
fn a_user_spec_inherits_the_bundled_defaults() {
    let sb = sandbox();
    sb.user_spec("mytool.yaml", MINIMAL);
    let spec = load_specs(&sb.settings)["mytool"].clone();
    assert_eq!((spec.map["prompt"][0].as_str(), spec.usage.events.clone()), ("prompt", vec!["api_request".to_string()]));
}

#[test]
fn user_spec_overrides_bundled() {
    let sb = sandbox();
    sb.user_spec("claude-code.yaml", &bundled("claude-code.yaml").replace("name: Claude Code", "name: Patched"));
    assert_eq!(load_specs(&sb.settings)["claude-code"].name, "Patched");
}

#[test]
fn files_say_where_each_spec_came_from() {
    let sb = sandbox();
    sb.user_spec("claude-code.yaml", &bundled("claude-code.yaml").replace("name: Claude Code", "name: Patched"));
    sb.user_spec("mytool.yaml", MINIMAL);
    let files = load_spec_set(&sb.settings).files;
    assert_eq!(files["claude-code"], user_file(&sb, "claude-code.yaml", true));
    assert_eq!(files["mytool"], user_file(&sb, "mytool.yaml", false));
    assert_eq!(
        (files["claude-code-otel"].origin, files["claude-code-otel"].path.ends_with("claude-code-otel.yaml")),
        (SpecOrigin::Bundled, true)
    );
}

#[test]
fn a_user_defaults_file_is_ignored() {
    let sb = sandbox();
    sb.user_spec("_defaults.yaml", "map: {prompt: [nothing]}\nbroken: [\n");
    let set = load_spec_set(&sb.settings);
    assert_eq!((ids(&set), set.problems.len()), (BUNDLED_IDS.to_vec(), 0));
    assert_eq!(set.specs["claude-code"].map["prompt"][0], "prompt");
}

#[test]
fn only_yaml_files_are_read() {
    let sb = sandbox();
    sb.user_spec("notes.txt", "not a spec");
    sb.user_spec("mytool.yml", MINIMAL);
    let set = load_spec_set(&sb.settings);
    assert_eq!((ids(&set), set.problems.len()), (BUNDLED_IDS.to_vec(), 0));
}

/// (case, text) of user files that can't be used.
fn broken_files() -> Vec<(&'static str, String)> {
    vec![
        ("not yaml", "id: [unclosed\n".to_string()),
        ("not a mapping", "- a list\n".to_string()),
        ("missing keys", "id: mytool\n".to_string()),
        ("unknown capability", format!("{MINIMAL}capabilities: [telepathy]\n")),
        ("unknown join", format!("{MINIMAL}join: guess\n")),
        ("unknown tool kind", format!("{MINIMAL}tool_kinds: {{Run: teleport}}\n")),
    ]
}

fn load_broken(text: &str) -> (Vec<String>, SpecProblem) {
    let sb = sandbox();
    sb.user_spec("mytool.yaml", text);
    let set = load_spec_set(&sb.settings);
    assert_eq!(set.problems.len(), 1, "{:?}", set.problems);
    (set.specs.keys().cloned().collect(), set.problems[0].clone())
}

#[test]
fn a_broken_user_file_is_reported_and_skipped() {
    let outcomes: Vec<(&str, bool)> = broken_files()
        .into_iter()
        .map(|(case, text)| {
            let (ids, problem) = load_broken(&text);
            let ok = ids == BUNDLED_IDS
                && problem.file.name == "mytool.yaml"
                && problem.file.origin == SpecOrigin::User
                && !problem.file.overrides
                && problem.error.contains("mytool.yaml")
                && problem.fallback.is_none();
            (case, ok)
        })
        .collect();
    let expected: Vec<(&str, bool)> = broken_files().into_iter().map(|(case, _)| (case, true)).collect();
    assert_eq!(outcomes, expected);
}

#[test]
fn a_user_file_that_is_not_utf8_is_reported_and_skipped() {
    let sb = sandbox();
    let path = sb.user_spec("mytool.yaml", "");
    std::fs::write(path, b"id: \xff\xfe\n").unwrap();
    let set = load_spec_set(&sb.settings);
    assert_eq!((ids(&set), problem_names(&set)), (BUNDLED_IDS.to_vec(), vec!["mytool.yaml"]));
}

#[test]
fn a_broken_override_falls_back_to_the_bundled_spec() {
    let sb = sandbox();
    sb.user_spec("claude-code.yaml", "id: claude-code\nname: [broken\n");
    let set = load_spec_set(&sb.settings);
    assert_eq!((set.specs["claude-code"].name.as_str(), set.files["claude-code"].origin), ("Claude Code", SpecOrigin::Bundled));
    assert_eq!(set.problems.len(), 1);
    let problem = &set.problems[0];
    assert_eq!((&problem.file, problem.fallback.as_deref()), (&user_file(&sb, "claude-code.yaml", true), Some("claude-code")));
}

#[test]
fn a_broken_override_is_shown_on_the_bundled_connector() {
    let sb = sandbox();
    sb.user_spec("copilot-cli.yaml", "id: copilot-cli\n");
    let set = load_spec_set(&sb.settings);
    assert_eq!(set.files["copilot-cli"].origin, SpecOrigin::Bundled);
    assert_eq!(set.problems.len(), 1);
    let problem = &set.problems[0];
    assert!(problem.file.overrides && problem.error.contains("copilot-cli.yaml"), "{problem:?}");
    assert_eq!(problem.fallback.as_deref(), Some("copilot-cli"));
}

#[test]
fn a_broken_user_spec_is_isolated_and_problems_are_listed_by_file_name() {
    let sb = sandbox();
    sb.user_spec("other.yaml", "id: [unclosed\n");
    sb.user_spec("mytool.yaml", "id: mytool\n");
    let set = load_spec_set(&sb.settings);
    assert_eq!((ids(&set), problem_names(&set)), (BUNDLED_IDS.to_vec(), vec!["mytool.yaml", "other.yaml"]));
    assert_eq!((set.problems[0].file.origin, set.problems[0].error.contains("mytool.yaml")), (SpecOrigin::User, true));
}

#[test]
fn a_duplicate_id_keeps_the_first_file() {
    let sb = sandbox();
    sb.user_spec("a.yaml", MINIMAL);
    sb.user_spec("b.yaml", &MINIMAL.replace("My tool", "Copy"));
    let set = load_spec_set(&sb.settings);
    assert_eq!(set.specs["mytool"].name, "My tool");
    assert_eq!(set.files["mytool"].name, "a.yaml");
    assert_eq!(set.problems.len(), 1);
    let problem = &set.problems[0];
    assert_eq!((problem.file.name.as_str(), problem.fallback.clone()), ("b.yaml", None));
    assert!(problem.error.contains("already used by a.yaml"), "{}", problem.error);
}

#[test]
fn a_user_spec_reusing_a_bundled_id_under_another_name_is_a_problem() {
    let sb = sandbox();
    sb.user_spec("zz-codex.yaml", &bundled("codex.yaml").replace("name: OpenAI Codex CLI", "name: Copy"));
    let set = load_spec_set(&sb.settings);
    assert_eq!(ids(&set), BUNDLED_IDS);
    assert_eq!(set.problems.len(), 1);
    assert!(set.problems[0].error.contains("already used by"), "{:?}", set.problems);
}

#[test]
fn part_of_must_name_a_main_spec() {
    let sb = sandbox();
    sb.user_spec("mytool-otel.yaml", &(MINIMAL.replace("id: mytool", "id: mytool-otel") + "part_of: nowhere\n"));
    let set = load_spec_set(&sb.settings);
    assert!(!set.specs.contains_key("mytool-otel") && !set.files.contains_key("mytool-otel") && set.specs.contains_key("claude-code-otel"));
    assert_eq!(problem_names(&set), ["mytool-otel.yaml"]);
    assert!(set.problems[0].error.contains("part_of"), "{}", set.problems[0].error);
}

#[test]
fn part_of_naming_a_layer_instead_of_a_main_spec_is_a_problem() {
    let sb = sandbox();
    sb.user_spec("layer.yaml", &(MINIMAL.replace("id: mytool", "id: layer") + "part_of: claude-code-otel\n"));
    let set = load_spec_set(&sb.settings);
    assert!(!set.specs.contains_key("layer"));
    assert_eq!(problem_names(&set), ["layer.yaml"]);
}

#[test]
fn part_of_naming_a_main_spec_loads_the_layer() {
    let sb = sandbox();
    sb.user_spec("mytool-otel.yaml", &(MINIMAL.replace("id: mytool", "id: mytool-otel") + "part_of: claude-code\n"));
    let set = load_spec_set(&sb.settings);
    assert_eq!((set.specs["mytool-otel"].part_of.as_deref(), set.problems.len()), (Some("claude-code"), 0));
}

// --- read_spec_file -------------------------------------------------------------------------

#[test]
fn reading_a_bundled_spec_file_gives_the_bundled_text() {
    let sb = sandbox();
    let file = load_spec_set(&sb.settings).files["cursor"].clone();
    assert_eq!(read_spec_file(&file).unwrap(), bundled("cursor.yaml"));
}

#[test]
fn reading_a_user_spec_file_gives_its_text() {
    let sb = sandbox();
    sb.user_spec("mytool.yaml", MINIMAL);
    assert_eq!(read_spec_file(&user_file(&sb, "mytool.yaml", false)).unwrap(), MINIMAL);
}

#[test]
fn reading_a_missing_user_spec_file_is_an_io_error() {
    let sb = sandbox();
    let err = read_spec_file(&user_file(&sb, "gone.yaml", false)).unwrap_err();
    assert!(matches!(err, Error::Io(_)), "{err:?}");
}

// --- SpecCache ------------------------------------------------------------------------------

/// Rewrite a file and push its mtime forward, so a change is visible even on coarse-mtime filesystems.
fn rewrite(path: &std::path::Path, text: &str, seconds_ahead: u64) {
    std::fs::write(path, text).unwrap();
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() + Duration::from_secs(seconds_ahead)).unwrap();
}

#[test]
fn the_cache_does_not_reload_when_nothing_changed() {
    let sb = sandbox();
    sb.user_spec("mytool.yaml", MINIMAL);
    let cache = SpecCache::new(sb.settings);
    let first = cache.get();
    let loads = cache.loads();
    let second = cache.get();
    assert_eq!(cache.loads(), loads);
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(ids(&second), ["claude-code", "claude-code-otel", "codex", "copilot-cli", "cursor", "mytool"]);
}

#[test]
fn the_cache_loads_from_disk_once_for_its_first_set() {
    let sb = sandbox();
    let cache = SpecCache::new(sb.settings);
    cache.get();
    cache.get();
    assert_eq!(cache.loads(), 1);
}

#[test]
fn the_cache_reloads_after_a_user_spec_is_added() {
    let sb = sandbox();
    std::fs::create_dir_all(sb.settings.user_sources()).unwrap();
    let cache = SpecCache::new(sb.settings.clone());
    assert!(cache.get().get("mytool").is_none());
    let loads = cache.loads();
    sb.user_spec("mytool.yaml", MINIMAL);
    assert_eq!(cache.get().get("mytool").map(|s| s.name.clone()), Some("My tool".to_string()));
    assert_eq!(cache.loads(), loads + 1);
}

#[test]
fn the_cache_reloads_when_the_sources_directory_appears() {
    let sb = sandbox();
    let cache = SpecCache::new(sb.settings.clone());
    cache.get();
    let loads = cache.loads();
    sb.user_spec("mytool.yaml", MINIMAL);
    assert!(cache.get().get("mytool").is_some());
    assert_eq!(cache.loads(), loads + 1);
}

#[test]
fn the_cache_reloads_after_a_user_spec_is_modified() {
    let sb = sandbox();
    let path = sb.user_spec("mytool.yaml", MINIMAL);
    let cache = SpecCache::new(sb.settings);
    assert_eq!(cache.get().specs["mytool"].name, "My tool");
    let loads = cache.loads();
    rewrite(&path, &MINIMAL.replace("My tool", "My renamed tool"), 5);
    assert_eq!(cache.get().specs["mytool"].name, "My renamed tool");
    assert_eq!(cache.loads(), loads + 1);
}

#[test]
fn the_cache_reloads_after_a_user_spec_is_removed() {
    let sb = sandbox();
    let path = sb.user_spec("mytool.yaml", MINIMAL);
    let cache = SpecCache::new(sb.settings);
    assert!(cache.get().get("mytool").is_some());
    let loads = cache.loads();
    std::fs::remove_file(path).unwrap();
    assert!(cache.get().get("mytool").is_none());
    assert_eq!(cache.loads(), loads + 1);
}

#[test]
fn the_cache_reports_a_broken_file_added_later() {
    let sb = sandbox();
    let cache = SpecCache::new(sb.settings.clone());
    assert_eq!(cache.get().problems.len(), 0);
    sb.user_spec("broken.yaml", "id: [unclosed\n");
    assert_eq!(problem_names(&cache.get()), ["broken.yaml"]);
}

#[test]
fn the_cache_is_shared_across_threads() {
    fn shareable<T: Send + Sync>() {}
    shareable::<SpecCache>();
    let sb = sandbox();
    let cache = SpecCache::new(sb.settings);
    let counts: Vec<usize> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4).map(|_| scope.spawn(|| cache.get().specs.len())).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(counts, [5, 5, 5, 5]);
}
