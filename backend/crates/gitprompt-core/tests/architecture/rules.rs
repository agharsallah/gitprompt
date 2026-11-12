//! The architecture map: the crates in layer order, what each may use, which external crates belong
//! to one layer, and which crates do no I/O. `main.rs` checks the workspace against it.
//!
//! Changing this file is a design decision, not a way to make a check pass: agree on it first, then
//! update `docs/extending.md` in the same change. A finding usually means the code is in the wrong
//! place (see "Architecture" in `docs/extending.md`).

/// A crate, what it is for, and the workspace crates it depends on (exactly: normal, build and dev
/// dependencies alike).
pub struct Layer {
    pub name: &'static str,
    pub role: &'static str,
    pub uses: &'static [&'static str],
}

/// Lowest first. A crate uses only crates listed above it, so crate cycles cannot exist.
/// `capture` and `insights` are siblings: neither knows the other.
pub const LAYERS: &[Layer] = &[
    Layer {
        name: "gitprompt-core",
        role: "shared kernel: settings, preferences, errors, JSON helpers, git facts, secrets, connector specs",
        uses: &[],
    },
    Layer { name: "gitprompt-dashboard", role: "the built web dashboard, embedded in the binary", uses: &[] },
    Layer {
        name: "gitprompt-store",
        role: "SQLite storage: raw prompts and events, the prompt index, statistics, housekeeping",
        uses: &["gitprompt-core"],
    },
    Layer {
        name: "gitprompt-capture",
        role: "writing side: hooks and OTLP in, tool probing, connect/disconnect, token rotation, spool",
        uses: &["gitprompt-core", "gitprompt-store"],
    },
    Layer {
        name: "gitprompt-insights",
        role: "reading side: values derived on read (origin, surface, turns, facts, stories), the indexer",
        uses: &["gitprompt-core", "gitprompt-store"],
    },
    Layer {
        name: "gitprompt-export",
        role: "exports: one redaction pass, report builders, renderers",
        uses: &["gitprompt-core", "gitprompt-store", "gitprompt-insights"],
    },
    Layer {
        name: "gitprompt-server",
        role: "HTTP API, live-update bus and the Service facade every front end calls",
        uses: &["gitprompt-core", "gitprompt-dashboard", "gitprompt-store", "gitprompt-capture", "gitprompt-insights", "gitprompt-export"],
    },
    Layer {
        name: "gitprompt-cli",
        role: "the gitprompt command",
        uses: &["gitprompt-core", "gitprompt-store", "gitprompt-capture", "gitprompt-insights", "gitprompt-server"],
    },
    Layer {
        name: "gitprompt-tauri",
        role: "Tauri plugin: the daemon in-process, native commands over the Service facade",
        uses: &["gitprompt-core", "gitprompt-store", "gitprompt-server"],
    },
    Layer { name: "gitprompt-desktop", role: "the desktop app", uses: &["gitprompt-core", "gitprompt-server", "gitprompt-tauri"] },
];

/// External crates that belong to one layer, with their `<name>-*` family (`tokio` covers
/// `tokio-util`). Normal and build dependencies only: a test may open a database or start a runtime.
pub const OWNED: &[(&str, &[&str])] = &[
    ("rusqlite", &["gitprompt-store"]),
    ("axum", &["gitprompt-server"]),
    ("tower", &["gitprompt-server"]),
    ("sysinfo", &["gitprompt-server"]),
    ("tracing-subscriber", &["gitprompt-server"]),
    ("tokio", &["gitprompt-server", "gitprompt-cli", "gitprompt-tauri"]),
    ("clap", &["gitprompt-cli"]),
    ("tauri", &["gitprompt-tauri", "gitprompt-desktop"]),
    ("resvg", &["gitprompt-export"]),
    ("rust-embed", &["gitprompt-dashboard"]),
];

/// The shared kernel's only external crates (normal and build dependencies): everything else
/// depends on core, so whatever it pulls in, every crate pulls in.
pub const KERNEL: (&str, &[&str]) =
    ("gitprompt-core", &["serde", "serde_json", "serde_norway", "indexmap", "thiserror", "fancy-regex", "getrandom", "tracing", "ts-rs"]);

/// Crates whose code does no I/O (files, processes, network, environment), each with the only
/// files under `src/` still allowed to. These lists only shrink: a listed file that stops doing
/// I/O fails the check until it is removed here. A crate missing from this list is an adapter.
pub const PURE: &[(&str, &[&str])] = &[
    // The kernel reads settings, preferences, specs and git facts itself.
    ("gitprompt-core", &["env.rs", "files.rs", "git.rs", "preferences.rs", "settings.rs", "spec.rs"]),
    ("gitprompt-insights", &[]),
    ("gitprompt-export", &[]),
];
