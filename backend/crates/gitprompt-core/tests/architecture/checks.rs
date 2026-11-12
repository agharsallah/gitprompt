//! The rules of `rules.rs`, each run over the workspace: its findings and what to do about them.
//! The tests fail on any finding; the report lists them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use fancy_regex::Regex;

use crate::rules::{COMPLEX, KERNEL, LAYERS, Layer, MAX_COMPLEXITY, MAX_LINES, OWNED, PURE};
use crate::source;
use crate::workspace::{Dep, Package, shown};
use crate::{complexity, modules};

pub const MAP: &str = "backend/crates/gitprompt-core/tests/architecture/rules.rs";

/// One rule's outcome.
#[derive(Debug)]
pub struct Check {
    pub rule: &'static str,
    pub advice: String,
    pub problems: Vec<String>,
}

impl Check {
    /// Fails with every problem at once, and what to do about them.
    pub fn assert_clean(&self) {
        assert!(self.problems.is_empty(), "\n{}\n\n{}\n", self.problems.join("\n"), self.advice);
    }
}

pub fn all(packages: &[Package]) -> Vec<Check> {
    vec![
        every_crate_has_a_layer(packages),
        the_map_is_layered(),
        crates_use_what_the_map_allows(packages),
        infrastructure_stays_in_its_layer(packages),
        pure_crates_do_no_io(packages),
        no_module_cycles(packages),
        files_stay_small(packages),
        functions_stay_simple(packages),
    ]
}

pub fn layer(name: &str) -> Option<&'static Layer> {
    LAYERS.iter().find(|layer| layer.name == name)
}

/// `dep` is `family` or one of its `family-*` crates.
fn in_family(dep: &str, family: &str) -> bool {
    dep == family || dep.strip_prefix(family).is_some_and(|rest| rest.starts_with('-'))
}

pub fn every_crate_has_a_layer(packages: &[Package]) -> Check {
    let members: BTreeSet<&str> = packages.iter().map(|package| package.name.as_str()).collect();
    let mapped: BTreeSet<&str> = LAYERS.iter().map(|layer| layer.name).collect();
    let mut problems: Vec<String> = members.difference(&mapped).map(|name| format!("{name} is not in LAYERS")).collect();
    problems.extend(mapped.difference(&members).map(|name| format!("LAYERS lists {name}, which is not a workspace member")));
    Check {
        rule: "every crate has a layer",
        advice: format!(
            "A new crate is a new layer or bounded context: agree on its place first, then add it to LAYERS in {MAP} and to docs/extending.md."
        ),
        problems,
    }
}

pub fn the_map_is_layered() -> Check {
    let mut problems = Vec::new();
    for (position, layer) in LAYERS.iter().enumerate() {
        let below: Vec<&str> = LAYERS[..position].iter().map(|layer| layer.name).collect();
        for used in layer.uses.iter().filter(|used| !below.contains(used)) {
            problems.push(format!("{} uses {used}, which is not listed above it", layer.name));
        }
    }
    let named = OWNED.iter().flat_map(|(_, owners)| owners.iter()).chain(PURE.iter().map(|(name, _)| name)).chain([&KERNEL.0]);
    problems.extend(named.filter(|name| layer(name).is_none()).map(|name| format!("{name} is named in {MAP} but has no layer")));
    Check {
        rule: "the map is layered",
        advice: "LAYERS is in dependency order, lowest first: a crate may only use crates above it, which keeps crate cycles impossible."
            .to_owned(),
        problems,
    }
}

pub fn crates_use_what_the_map_allows(packages: &[Package]) -> Check {
    let members: BTreeSet<&str> = packages.iter().map(|package| package.name.as_str()).collect();
    let mut problems = Vec::new();
    for package in packages {
        let Some(own) = layer(&package.name) else { continue };
        let used: BTreeSet<&str> = package.deps.iter().map(|dep| dep.name.as_str()).filter(|name| members.contains(name)).collect();
        let allowed: BTreeSet<&str> = own.uses.iter().copied().collect();
        for extra in used.difference(&allowed) {
            let role = layer(extra).map_or("not in the map", |used| used.role);
            problems.push(format!("{} ({}) depends on {extra} ({role}), which LAYERS does not let it use", package.name, own.role));
        }
        for missing in allowed.difference(&used) {
            problems.push(format!("LAYERS lets {} use {missing}, but it no longer does: remove it from the map", package.name));
        }
    }
    Check {
        rule: "crates use only what the map allows",
        advice: "A dependency the map does not allow means the code is in the wrong crate: move what both need down into the lower crate \
                 (or a new one below both), or invert it with a trait the lower crate defines and the upper one implements. \
                 Changing the map itself is a design decision: ask first."
            .to_owned(),
        problems,
    }
}

pub fn infrastructure_stays_in_its_layer(packages: &[Package]) -> Check {
    let problems = packages.iter().flat_map(|package| package.shipped().flat_map(move |dep| misplaced_dependency(package, dep))).collect();
    Check {
        rule: "infrastructure crates stay in their layer",
        advice: "SQL belongs to the store, HTTP and the async runtime to the server and its front ends, Tauri to the desktop crates. \
                 Call the owning crate's API instead, or move the code there. Everything depends on core, so core stays small."
            .to_owned(),
        problems,
    }
}

/// Why `package` may not depend on `dep`: an infrastructure crate owned by other layers, or, in the
/// shared kernel, a crate KERNEL does not list.
fn misplaced_dependency(package: &Package, dep: &Dep) -> Vec<String> {
    let user = package.name.as_str();
    let mut problems: Vec<String> = OWNED
        .iter()
        .filter(|(family, owners)| in_family(&dep.name, family) && !owners.contains(&user))
        .map(|(_, owners)| format!("{user} depends on {} (owned by {})", dep.name, owners.join(", ")))
        .collect();
    if user == KERNEL.0 && !KERNEL.1.contains(&dep.name.as_str()) {
        problems.push(format!("the shared kernel {user} depends on {}, which KERNEL does not list", dep.name));
    }
    problems
}

/// Files, processes, network and environment access.
fn io_pattern() -> Regex {
    Regex::new(r"\bstd::(?:fs|process|net|env)\b|\bstd::\{[^}]*\b(?:fs|process|net|env)\b|\bCommand::new\b|\btokio::(?:fs|process|net)\b")
        .unwrap()
}

pub fn pure_crates_do_no_io(packages: &[Package]) -> Check {
    let io = io_pattern();
    let mut problems = Vec::new();
    for (name, allowed) in PURE {
        let Some(package) = packages.iter().find(|package| package.name == *name) else { continue };
        for src in &package.src_dirs {
            problems.extend(io_problems(&io, name, allowed, src));
        }
    }
    Check {
        rule: "pure crates do no I/O",
        advice: "Pure crates compute; adapters (store, capture, server, cli) do the I/O. Take the data as an argument, or a trait the \
                 caller implements (like `Env`, `gitprompt-capture::probe`), and do the I/O in the crate that calls it."
            .to_owned(),
        problems,
    }
}

/// The files under `src` that do I/O although `allowed` does not list them, and the files `allowed`
/// lists that no longer do.
fn io_problems(io: &Regex, name: &str, allowed: &[&str], src: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    let mut doing_io = BTreeSet::new();
    for file in source::rust_files(src) {
        let code = source::read_code(&file);
        let Some(found) = io.find(&code).unwrap() else { continue };
        let relative = file.strip_prefix(src).unwrap().to_string_lossy().into_owned();
        if !allowed.contains(&relative.as_str()) {
            let line = source::line_of(&code, found.start());
            problems.push(format!("{}:{line} does I/O (`{}`), but {name} is pure", shown(&file), found.as_str()));
        }
        doing_io.insert(relative);
    }
    problems.extend(
        allowed
            .iter()
            .filter(|file| !doing_io.contains(**file))
            .map(|gone| format!("{name}/src/{gone} no longer does I/O: remove it from PURE in {MAP}")),
    );
    problems
}

pub fn no_module_cycles(packages: &[Package]) -> Check {
    let mut problems = Vec::new();
    for package in packages {
        for src in &package.src_dirs {
            let graph = modules::graph(src);
            problems.extend(modules::cycles(&graph).iter().map(|cycle| describe_cycle(&package.name, &graph, cycle)));
        }
    }
    Check {
        rule: "no module cycles",
        advice: "Modules of a crate form layers too. Break the loop by moving the shared item to the module that owns the concept \
                 (usually the lower one), or into a new small module both use. Never merge the modules to hide it."
            .to_owned(),
        problems,
    }
}

/// A module cycle of crate `name`, with the `use` behind each of its edges.
fn describe_cycle(name: &str, graph: &modules::Graph, cycle: &[String]) -> String {
    let edges: Vec<String> = cycle
        .iter()
        .flat_map(|from| {
            graph[from].iter().filter(|(to, _)| cycle.contains(to)).map(move |(to, site)| format!("    {from} -> {to}  ({site})"))
        })
        .collect();
    format!("{name}: {} use each other\n{}", cycle.join(", "), edges.join("\n"))
}

/// The crate's `src/` (shipped) and `tests/` folders, each with its line limit.
fn sized_folders(package: &Package) -> Vec<(PathBuf, usize)> {
    let (shipped, tests) = MAX_LINES;
    let mut folders: Vec<(PathBuf, usize)> = package.src_dirs.iter().map(|src| (src.clone(), shipped)).collect();
    let crate_dirs: BTreeSet<&Path> = package.src_dirs.iter().filter_map(|src| src.parent()).collect();
    folders.extend(crate_dirs.into_iter().map(|dir| dir.join("tests")).filter(|dir| dir.is_dir()).map(|dir| (dir, tests)));
    folders
}

pub fn files_stay_small(packages: &[Package]) -> Check {
    let mut problems = Vec::new();
    for (folder, max) in packages.iter().flat_map(sized_folders) {
        for file in source::rust_files(&folder) {
            let lines = std::fs::read_to_string(&file).unwrap().lines().count();
            if lines > max {
                problems.push(format!("{} has {lines} lines (at most {max})", shown(&file)));
            }
        }
    }
    Check {
        rule: "files stay small",
        advice: format!(
            "A long file holds several ideas. Split it by concept into a module folder (`x.rs` -> `x/mod.rs` plus one file per \
             part, each with one job); split tests by the behaviour they cover. MAX_LINES in {MAP} only goes down."
        ),
        problems,
    }
}

/// Every function of the crates' `src/` and `tests/` folders, measured (`complexity.rs`).
pub fn measured_functions(packages: &[Package]) -> Vec<complexity::Function> {
    let folders = packages.iter().flat_map(sized_folders).map(|(folder, _)| folder);
    folders.flat_map(|folder| source::rust_files(&folder)).flat_map(|file| complexity::functions(&file)).collect()
}

pub fn functions_stay_simple(packages: &[Package]) -> Check {
    let functions = measured_functions(packages);
    let key = |f: &complexity::Function| format!("{}::{}", shown(&f.file), f.name);
    let over: Vec<&complexity::Function> = functions.iter().filter(|f| f.complexity > MAX_COMPLEXITY).collect();
    let allowed = |f: &&complexity::Function| COMPLEX.iter().any(|(name, _)| *name == key(f));
    let mut problems: Vec<String> = over
        .iter()
        .filter(|f| !allowed(f))
        .map(|f| format!("{}:{} {} has cognitive complexity {} (at most {MAX_COMPLEXITY})", shown(&f.file), f.line, f.name, f.complexity))
        .collect();
    let still_over: BTreeSet<String> = over.iter().map(|f| key(f)).collect();
    problems.extend(
        COMPLEX
            .iter()
            .filter(|(name, _)| !still_over.contains(*name))
            .map(|(name, _)| format!("COMPLEX lists {name}, which is within the limit now (or gone): remove it")),
    );
    Check {
        rule: "functions stay simple",
        advice: format!(
            "A function over the limit does several things or hides its flow in nesting. Split it by step or concept, return early, \
             turn nested loops into iterator chains or lookups. Only logic that is inherently that complex goes into COMPLEX in \
             {MAP}, with the reason, after the user agrees. MAX_COMPLEXITY only goes down."
        ),
        problems,
    }
}
