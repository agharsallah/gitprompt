//! A crate's top-level modules and which of them use which, for the cycle check. A module is
//! `src/x.rs` plus everything under `src/x/`. `x` uses `y` when its shipped code names `crate::y`
//! (also inside `crate::{…}`), reaches `y` through enough `super::` to get to the crate root, or
//! names an item the crate root re-exports from `y`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::source::{is_ident, line_of, read_code, rust_files};
use crate::workspace::shown;

/// module -> module it uses -> where (the first `file:line` naming it).
pub type Graph = BTreeMap<String, BTreeMap<String, String>>;

pub fn graph(src: &Path) -> Graph {
    let files = rust_files(src);
    let modules: BTreeSet<String> = files.iter().filter_map(|file| top_module(src, file)).collect();
    let reexports = reexports(src, &modules);
    let mut graph: Graph = modules.iter().map(|module| (module.clone(), BTreeMap::new())).collect();
    for file in &files {
        let Some(module) = top_module(src, file) else { continue };
        let code = read_code(file);
        for (offset, name) in root_names(&code, depth(src, file)) {
            let used = if modules.contains(&name) { Some(name) } else { reexports.get(&name).cloned() };
            if let Some(used) = used.filter(|used| *used != module) {
                let site = || format!("{}:{}", shown(file), line_of(&code, offset));
                graph.get_mut(&module).unwrap().entry(used).or_insert_with(site);
            }
        }
    }
    graph
}

/// The modules caught in a cycle, one sorted list per cycle (strongly connected components).
pub fn cycles(graph: &Graph) -> Vec<Vec<String>> {
    let mut tarjan = Tarjan { graph, index: BTreeMap::new(), low: BTreeMap::new(), stack: Vec::new(), found: Vec::new() };
    for module in graph.keys() {
        if !tarjan.index.contains_key(module.as_str()) {
            tarjan.visit(module);
        }
    }
    tarjan.found
}

struct Tarjan<'a> {
    graph: &'a Graph,
    index: BTreeMap<&'a str, usize>,
    low: BTreeMap<&'a str, usize>,
    stack: Vec<&'a str>,
    found: Vec<Vec<String>>,
}

impl<'a> Tarjan<'a> {
    fn visit(&mut self, module: &'a str) {
        let index = self.index.len();
        self.index.insert(module, index);
        self.low.insert(module, index);
        self.stack.push(module);
        for used in self.graph[module].keys() {
            let used = used.as_str();
            if !self.index.contains_key(used) {
                self.visit(used);
                self.low.insert(module, self.low[module].min(self.low[used]));
            } else if self.stack.contains(&used) {
                self.low.insert(module, self.low[module].min(self.index[used]));
            }
        }
        if self.low[module] == index {
            let at = self.stack.iter().rposition(|&m| m == module).unwrap();
            let mut cycle: Vec<String> = self.stack.drain(at..).map(str::to_owned).collect();
            if cycle.len() > 1 {
                cycle.sort();
                self.found.push(cycle);
            }
        }
    }
}

/// `x` for `src/x.rs` and `src/x/**`; none for the crate roots and `src/bin/` (separate crates).
pub fn top_module(src: &Path, file: &Path) -> Option<String> {
    let parts = parts(src, file);
    match parts.as_slice() {
        [root] if root == "lib.rs" || root == "main.rs" => None,
        [file] => file.strip_suffix(".rs").map(str::to_owned),
        [dir, ..] if dir == "bin" => None,
        [dir, ..] => Some(dir.clone()),
        [] => None,
    }
}

/// How many `super::` lead from the file's module to the crate root.
fn depth(src: &Path, file: &Path) -> usize {
    let parts = parts(src, file);
    parts.len() - usize::from(parts.last().is_some_and(|last| last == "mod.rs"))
}

fn parts(src: &Path, file: &Path) -> Vec<String> {
    file.strip_prefix(src).unwrap().iter().map(|part| part.to_string_lossy().into_owned()).collect()
}

/// Names the code reaches at the crate root, with their offset: `crate::name`, the items of
/// `crate::{a, b::c}`, and the same after `depth` times `super::`.
fn root_names(code: &str, depth: usize) -> Vec<(usize, String)> {
    let mut names = Vec::new();
    for (at, _) in code.match_indices("crate::") {
        if starts_path(code, at) {
            names.extend(names_after(code, at + "crate::".len()).into_iter().map(|name| (at, name)));
        }
    }
    for (at, _) in code.match_indices("super::") {
        if !starts_path(code, at) {
            continue;
        }
        let supers = code.as_bytes()[at..].chunks("super::".len()).take_while(|chunk| *chunk == b"super::").count();
        if supers == depth {
            names.extend(names_after(code, at + supers * "super::".len()).into_iter().map(|name| (at, name)));
        }
    }
    names
}

/// Whether a path starts at `at` (not `x::crate` or `my_crate::`).
fn starts_path(code: &str, at: usize) -> bool {
    code[..at].chars().next_back().is_none_or(|c| !is_ident(c) && c != ':')
}

/// The first segment of the path at `from`, or of each item when it is a `{…}` group.
fn names_after(code: &str, from: usize) -> Vec<String> {
    let rest = code[from..].trim_start();
    let Some(group) = rest.strip_prefix('{') else { return leading_ident(rest).into_iter().collect() };
    let mut depth = 0;
    let mut items = vec![String::new()];
    for c in group.chars() {
        match c {
            '{' => depth += 1,
            '}' if depth == 0 => break,
            '}' => depth -= 1,
            ',' if depth == 0 => items.push(String::new()),
            _ => {}
        }
        if let Some(item) = items.last_mut() {
            item.push(c);
        }
    }
    items.iter().filter_map(|item| leading_ident(item.trim_start_matches(',').trim_start())).filter(|name| name != "self").collect()
}

fn leading_ident(text: &str) -> Option<String> {
    let ident: String = text.chars().take_while(|&c| is_ident(c)).collect();
    (!ident.is_empty()).then_some(ident)
}

/// Item name -> module, for each `use` in the crate roots that brings an item of a module to the
/// root (`pub use origin::{Classified, Origin as Kind};`).
fn reexports(src: &Path, modules: &BTreeSet<String>) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    for root in ["lib.rs", "main.rs"].map(|name| src.join(name)).iter().filter(|path| path.exists()) {
        let code = read_code(root);
        for (at, _) in code.match_indices("use ") {
            if !starts_path(&code, at) {
                continue;
            }
            let body = code[at + "use ".len()..].split(';').next().unwrap_or_default().trim();
            let body = body.trim_start_matches("crate::").trim_start_matches("self::");
            let Some((module, tree)) = body.split_once("::") else { continue };
            if modules.contains(module) {
                for name in leaf_names(tree) {
                    found.insert(name, module.to_owned());
                }
            }
        }
    }
    found
}

/// The names a use tree binds: the last segment of each leaf, or its `as` alias.
fn leaf_names(tree: &str) -> Vec<String> {
    tree.split(['{', '}', ','])
        .map(str::trim)
        .filter_map(|leaf| leaf.rsplit(" as ").next()?.rsplit("::").next())
        .map(str::trim)
        .filter(|name| !name.is_empty() && *name != "*" && *name != "self")
        .map(str::to_owned)
        .collect()
}

#[test]
fn names_reached_at_the_root_are_found() {
    let names = |code: &str, depth| root_names(code, depth).into_iter().map(|(_, name)| name).collect::<Vec<_>>();
    assert_eq!(names("use crate::a::B; crate::c(); use crate::{d::{E, F}, g, self};", 2), ["a", "c", "d", "g"]);
    assert_eq!(names("use super::super::h; use super::i;", 2), ["h"], "only a path that reaches the root");
    assert_eq!(names("use super::{j, k};", 1), ["j", "k"]);
    assert_eq!(names("use my_crate::x; foo::crate::y;", 1), Vec::<String>::new());
}

#[test]
fn re_exported_names_point_at_their_module() {
    assert_eq!(leaf_names("{Classified, Origin as Kind, inner::Deep, *}"), ["Classified", "Kind", "Deep"]);
    assert_eq!(leaf_names("Store"), ["Store"]);
}
