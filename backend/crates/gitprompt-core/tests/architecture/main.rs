//! The workspace's architecture, checked against the map in `rules.rs`: every crate has a layer, a
//! crate depends only on the crates the map lets it use, infrastructure crates stay in the layer
//! that owns them, pure crates do no I/O, and no crate's modules use each other in a loop.
//!
//! It reads the whole workspace (`cargo metadata` and the sources) but lives in the lowest crate so
//! `just arch` compiles in seconds. What to do about a finding: "Architecture" in docs/extending.md.
//! `just arch-report` also draws the map and scores how modular the code is (`metrics.rs`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, the score is a clamped 0-100, and helpers are crate-private anyway"
)]

mod checks;
mod coverage;
mod metrics;
mod modules;
mod report;
mod rules;
mod source;
mod workspace;

use workspace::packages;

#[test]
fn every_crate_has_a_layer_and_every_layer_a_crate() {
    checks::every_crate_has_a_layer(&packages()).assert_clean();
}

#[test]
fn the_map_is_layered() {
    checks::the_map_is_layered().assert_clean();
}

#[test]
fn crates_depend_only_on_what_the_map_allows() {
    checks::crates_use_what_the_map_allows(&packages()).assert_clean();
}

#[test]
fn infrastructure_crates_stay_in_their_layer() {
    checks::infrastructure_stays_in_its_layer(&packages()).assert_clean();
}

#[test]
fn pure_crates_do_no_io() {
    checks::pure_crates_do_no_io(&packages()).assert_clean();
}

#[test]
fn no_crate_has_a_module_cycle() {
    checks::no_module_cycles(&packages()).assert_clean();
}

/// `just arch-report` (`ARCH_REPORT=1`): the map drawn from the code, with the modularity score.
/// Writes into `backend/target/architecture/` and never fails on findings: the tests above do.
#[test]
fn report() {
    if std::env::var_os("ARCH_REPORT").is_none() {
        return;
    }
    let packages = packages();
    let measured = coverage::load();
    let report = metrics::report(&packages, checks::all(&packages), measured.as_ref());
    let page = report::write(&report);
    eprintln!("modularity score {}/100", report.score.total);
    for part in &report.score.parts {
        eprintln!("  {:<16} {:>3.0}%  {}", part.name, part.value * 100.0, part.detail);
    }
    match &report.coverage {
        Some(c) => eprintln!(
            "  line coverage    {:>3.0}%  {} of {} lines (just cov)",
            c.lines.covered as f64 * 100.0 / c.lines.count.max(1) as f64,
            c.lines.covered,
            c.lines.count
        ),
        None => eprintln!("  line coverage      -   not measured: `just quality` runs coverage first"),
    }
    eprintln!("report: {}", page.display());
}
