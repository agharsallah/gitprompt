//! The rules of `rules.rs`, each run over the workspace: its findings and what to do about them.
//! The tests fail on any finding; the report lists them.

use std::collections::BTreeSet;

use fancy_regex::Regex;

use crate::modules;
use crate::rules::{KERNEL, LAYERS, Layer, OWNED, PURE};
use crate::source;
use crate::workspace::{Package, shown};

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
    let mut problems = Vec::new();
    for package in packages {
        for dep in package.shipped() {
            for (_, owners) in OWNED.iter().filter(|(family, _)| in_family(&dep.name, family)) {
                if !owners.contains(&package.name.as_str()) {
                    problems.push(format!("{} depends on {} (owned by {})", package.name, dep.name, owners.join(", ")));
                }
            }
            if package.name == KERNEL.0 && !KERNEL.1.contains(&dep.name.as_str()) {
                problems.push(format!("the shared kernel {} depends on {}, which KERNEL does not list", package.name, dep.name));
            }
        }
    }
    Check {
        rule: "infrastructure crates stay in their layer",
        advice: "SQL belongs to the store, HTTP and the async runtime to the server and its front ends, Tauri to the desktop crates. \
                 Call the owning crate's API instead, or move the code there. Everything depends on core, so core stays small."
            .to_owned(),
        problems,
    }
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
            let mut doing_io = BTreeSet::new();
            for file in source::rust_files(src) {
                let relative = file.strip_prefix(src).unwrap().to_string_lossy().into_owned();
                let code = source::read_code(&file);
                let Some(found) = io.find(&code).unwrap() else { continue };
                doing_io.insert(relative.clone());
                if !allowed.contains(&relative.as_str()) {
                    let line = source::line_of(&code, found.start());
                    problems.push(format!("{}:{line} does I/O (`{}`), but {name} is pure", shown(&file), found.as_str()));
                }
            }
            for gone in allowed.iter().filter(|file| !doing_io.contains(**file)) {
                problems.push(format!("{name}/src/{gone} no longer does I/O: remove it from PURE in {MAP}"));
            }
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

pub fn no_module_cycles(packages: &[Package]) -> Check {
    let mut problems = Vec::new();
    for package in packages {
        for src in &package.src_dirs {
            let graph = modules::graph(src);
            for cycle in modules::cycles(&graph) {
                let edges = cycle.iter().flat_map(|from| {
                    graph[from].iter().filter(|(to, _)| cycle.contains(to)).map(move |(to, site)| format!("    {from} -> {to}  ({site})"))
                });
                problems.push(format!("{}: {} use each other\n{}", package.name, cycle.join(", "), edges.collect::<Vec<_>>().join("\n")));
            }
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
