//! The numbers behind the architecture report: size, coupling and stability of each crate and its
//! modules, and one modularity score built from published measures:
//!
//! * propagation cost (MacCormack, Rusnak & Baldwin, 2006): the share of module pairs where a change
//!   in one can reach the other through dependencies. Lower is more modular.
//! * instability, abstractness and distance from the main sequence (Robert C. Martin, 1994), per
//!   crate. Shown, not scored: they mark a concrete shared kernel such as core as a risk by design.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fancy_regex::Regex;
use serde::Serialize;

use crate::checks::{Check, layer};
use crate::coverage::{self, Coverage, Lines};
use crate::modules;
use crate::rules::LAYERS;
use crate::source;
use crate::workspace::{Package, shown};

/// Code lines (not counting comments, blank lines and tests) above which a file counts as too big.
pub const BIG_FILE: usize = 400;
/// Largest files listed per crate.
const LISTED_FILES: usize = 8;

#[derive(Debug, Serialize)]
pub struct Report {
    pub score: Score,
    pub checks: Vec<CheckOut>,
    pub crates: Vec<CrateStats>,
    /// Line coverage of the last `just cov`, when there is one.
    pub coverage: Option<CoverageOut>,
}

#[derive(Debug, Serialize)]
pub struct CoverageOut {
    pub lines: Lines,
    /// Seconds since the epoch.
    pub measured_at: u64,
    /// The line-by-line report, relative to the page.
    pub html: &'static str,
}

#[derive(Debug, Serialize)]
pub struct CheckOut {
    pub rule: &'static str,
    pub advice: String,
    pub problems: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Score {
    /// 0 to 100.
    pub total: u32,
    pub parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
pub struct Part {
    pub name: &'static str,
    pub weight: f64,
    /// 0 (worst) to 1 (best).
    pub value: f64,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct CrateStats {
    pub name: String,
    pub role: &'static str,
    /// Longest chain of crates below it (0: uses no workspace crate).
    pub depth: usize,
    pub uses: Vec<String>,
    pub used_by: Vec<String>,
    /// Ce / (Ca + Ce): 0 when only used, 1 when only using.
    pub instability: f64,
    /// Traits among the types it defines.
    pub abstractness: f64,
    /// |A + I - 1|: how far from the balance of abstract-and-stable or concrete-and-unstable.
    pub distance: f64,
    pub files: usize,
    pub lines: usize,
    pub propagation_cost: f64,
    pub coverage: Option<Lines>,
    pub modules: Vec<ModuleStats>,
    /// Code lines in files over [`BIG_FILE`].
    pub big_lines: usize,
    pub largest_files: Vec<FileStats>,
}

#[derive(Debug, Serialize)]
pub struct ModuleStats {
    pub name: String,
    pub depth: usize,
    pub files: usize,
    pub lines: usize,
    /// Modules it uses, each with the first place it does.
    pub uses: BTreeMap<String, String>,
    pub used_by: usize,
    pub coverage: Option<Lines>,
}

#[derive(Debug, Serialize)]
pub struct FileStats {
    pub path: String,
    pub lines: usize,
    pub coverage: Option<Lines>,
}

pub fn report(packages: &[Package], checks: Vec<Check>, coverage: Option<&Coverage>) -> Report {
    let crates: Vec<CrateStats> = LAYERS
        .iter()
        .filter_map(|layer| packages.iter().find(|package| package.name == layer.name))
        .map(|package| crate_stats(package, coverage))
        .collect();
    let checks: Vec<CheckOut> = checks.into_iter().map(|c| CheckOut { rule: c.rule, advice: c.advice, problems: c.problems }).collect();
    let coverage = coverage.and_then(|measured| {
        let lines = total(crates.iter().map(|c| c.coverage))?;
        Some(CoverageOut { lines, measured_at: measured.measured_at, html: coverage::HTML_FROM_REPORT })
    });
    Report { score: score(&checks, &crates), checks, crates, coverage }
}

/// The sum of the measured parts; `None` when none was measured.
fn total(parts: impl Iterator<Item = Option<Lines>>) -> Option<Lines> {
    parts.flatten().reduce(Lines::add)
}

fn score(checks: &[CheckOut], crates: &[CrateStats]) -> Score {
    let findings: usize = checks.iter().map(|check| check.problems.len()).sum();
    let modules: usize = crates.iter().map(|c| c.modules.len()).sum();
    let coupling = crates.iter().map(|c| c.propagation_cost * c.modules.len() as f64).sum::<f64>() / modules.max(1) as f64;
    let lines: usize = crates.iter().map(|c| c.lines).sum();
    let big: usize = crates.iter().map(|c| c.big_lines).sum();
    let parts = vec![
        Part {
            name: "Rules",
            weight: 0.4,
            value: 1.0 - (findings as f64 / 10.0).min(1.0),
            detail: format!("{findings} findings across {} rules (each costs a tenth)", checks.len()),
        },
        Part {
            name: "Module coupling",
            weight: 0.35,
            value: 1.0 - coupling,
            detail: format!("propagation cost {:.0}% over {modules} modules, weighted by module count", coupling * 100.0),
        },
        Part {
            name: "File size",
            weight: 0.25,
            value: 1.0 - big as f64 / lines.max(1) as f64,
            detail: format!("{big} of {lines} code lines are in files over {BIG_FILE} lines"),
        },
    ];
    let total = parts.iter().map(|part| part.weight * part.value).sum::<f64>() * 100.0;
    Score { total: total.round().clamp(0.0, 100.0) as u32, parts }
}

/// Every crate here has one source folder (`src/`, holding `lib.rs` and maybe `main.rs`).
fn crate_stats(package: &Package, coverage: Option<&Coverage>) -> CrateStats {
    let own = layer(&package.name).unwrap();
    // Canonical, like the coverage paths.
    let src = &package.src_dirs[0].canonicalize().unwrap();
    let covered = |path: &str| coverage.and_then(|c| c.of(&src.join(path)));
    let used_by: Vec<String> = LAYERS.iter().filter(|other| other.uses.contains(&own.name)).map(|other| other.name.to_owned()).collect();
    let (ca, ce) = (used_by.len() as f64, own.uses.len() as f64);
    let instability = if ca + ce == 0.0 { 0.0 } else { ce / (ca + ce) };
    let (traits, types) = type_counts(src);
    let abstractness = if types == 0 { 0.0 } else { traits as f64 / types as f64 };
    let graph = modules::graph(src);
    let sized = sized_files(src);
    let mut files: Vec<FileStats> =
        sized.iter().map(|(path, lines)| FileStats { path: shown(&src.join(path)), lines: *lines, coverage: covered(path) }).collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.lines));
    CrateStats {
        name: package.name.clone(),
        role: own.role,
        depth: crate_depths()[own.name],
        uses: own.uses.iter().map(|&name| name.to_owned()).collect(),
        used_by,
        instability,
        abstractness,
        distance: (abstractness + instability - 1.0).abs(),
        files: files.len(),
        lines: files.iter().map(|f| f.lines).sum(),
        propagation_cost: propagation_cost(&graph),
        coverage: total(files.iter().map(|f| f.coverage)),
        modules: module_stats_of(src, &graph, &sized, &files),
        big_lines: files.iter().filter(|f| f.lines > BIG_FILE).map(|f| f.lines).sum(),
        largest_files: files.into_iter().take(LISTED_FILES).collect(),
    }
}

/// (file under `src`, code lines) for every file.
fn sized_files(src: &Path) -> Vec<(String, usize)> {
    source::rust_files(src)
        .into_iter()
        .map(|file| {
            let lines = source::read_code(&file).lines().filter(|line| !line.trim().is_empty()).count();
            (file.strip_prefix(src).unwrap().to_string_lossy().into_owned(), lines)
        })
        .collect()
}

/// (traits, all type definitions: traits, structs, enums) in the crate's shipped code.
fn type_counts(src: &Path) -> (usize, usize) {
    let definition = Regex::new(r"\b(trait|struct|enum)\s+[A-Za-z_]").unwrap();
    source::rust_files(src).iter().map(|file| source::read_code(file)).fold((0, 0), |(traits, all), code| {
        let kinds: Vec<String> = definition.captures_iter(&code).map(|c| c.unwrap().get(1).unwrap().as_str().to_owned()).collect();
        (traits + kinds.iter().filter(|k| *k == "trait").count(), all + kinds.len())
    })
}

fn module_stats_of(src: &Path, graph: &modules::Graph, sized: &[(String, usize)], files: &[FileStats]) -> Vec<ModuleStats> {
    let edges: BTreeMap<String, Vec<String>> = graph.iter().map(|(m, uses)| (m.clone(), uses.keys().cloned().collect())).collect();
    let depths = depths(&edges);
    graph
        .iter()
        .map(|(name, uses)| {
            let in_module = |path: &str| modules::top_module(src, &src.join(path)).as_deref() == Some(name.as_str());
            let mine: Vec<usize> = sized.iter().filter(|(path, _)| in_module(path)).map(|(_, lines)| *lines).collect();
            let shown_paths: Vec<String> =
                sized.iter().filter(|(path, _)| in_module(path)).map(|(path, _)| shown(&src.join(path))).collect();
            let coverage = total(files.iter().filter(|f| shown_paths.contains(&f.path)).map(|f| f.coverage));
            ModuleStats {
                name: name.clone(),
                depth: depths[name],
                files: mine.len(),
                lines: mine.iter().sum(),
                uses: uses.clone(),
                used_by: graph.values().filter(|other| other.contains_key(name)).count(),
                coverage,
            }
        })
        .collect()
}

fn crate_depths() -> BTreeMap<String, usize> {
    let edges = LAYERS.iter().map(|l| (l.name.to_owned(), l.uses.iter().map(|&u| u.to_owned()).collect())).collect();
    depths(&edges)
}

/// Longest path from each node down to a node that uses nothing (a cycle counts once).
fn depths(edges: &BTreeMap<String, Vec<String>>) -> BTreeMap<String, usize> {
    fn visit(node: &str, edges: &BTreeMap<String, Vec<String>>, done: &mut BTreeMap<String, usize>, path: &mut BTreeSet<String>) -> usize {
        if let Some(&depth) = done.get(node) {
            return depth;
        }
        if !path.insert(node.to_owned()) {
            return 0;
        }
        let below = edges.get(node).into_iter().flatten().map(|next| visit(next, edges, done, path) + 1).max().unwrap_or(0);
        path.remove(node);
        done.insert(node.to_owned(), below);
        below
    }
    let mut done = BTreeMap::new();
    for node in edges.keys() {
        visit(node, edges, &mut done, &mut BTreeSet::new());
    }
    done
}

/// Share of ordered module pairs (a, b), a ≠ b, where a reaches b through its dependencies.
fn propagation_cost(graph: &modules::Graph) -> f64 {
    let n = graph.len();
    if n < 2 {
        return 0.0;
    }
    let reached: usize = graph.keys().map(|start| reachable(graph, start)).sum();
    reached as f64 / (n * (n - 1)) as f64
}

fn reachable(graph: &modules::Graph, start: &str) -> usize {
    let mut seen = BTreeSet::from([start]);
    let mut todo = vec![start];
    while let Some(module) = todo.pop() {
        for next in graph.get(module).into_iter().flat_map(BTreeMap::keys) {
            if seen.insert(next) {
                todo.push(next);
            }
        }
    }
    seen.len() - 1
}

#[test]
fn propagation_cost_counts_what_a_change_can_reach() {
    let graph = |edges: &[(&str, &str)]| {
        let mut graph = modules::Graph::new();
        for (from, to) in edges {
            graph.entry((*from).to_owned()).or_default().insert((*to).to_owned(), String::new());
            graph.entry((*to).to_owned()).or_default();
        }
        graph
    };
    // a -> b -> c reaches 3 of the 6 ordered pairs.
    assert!((propagation_cost(&graph(&[("a", "b"), ("b", "c")])) - 0.5).abs() < 1e-9);
    assert!(propagation_cost(&graph(&[("a", "b"), ("c", "b")])) < 0.5, "a fan-in onto one module is cheaper than a chain");
}
