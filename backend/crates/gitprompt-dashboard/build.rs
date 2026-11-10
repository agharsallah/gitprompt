//! Re-embed the dashboard after `pnpm --dir web build`.
//!
//! Release builds compile `web/build` into the binary; `include_bytes!` notices edited files but not
//! new or removed ones, so the whole folder is watched. Debug builds read the folder at run time
//! and need no rebuild, so they watch nothing: cargo re-runs a build script on every build while a
//! watched path is missing, which would rebuild every crate above this one.

const BUILD_DIR: &str = "../../../web/build";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_CFG_DEBUG_ASSERTIONS").is_some() {
        return;
    }
    println!("cargo:rerun-if-changed={BUILD_DIR}");
    if !std::path::Path::new(BUILD_DIR).join("index.html").is_file() {
        println!("cargo:warning=the dashboard is not built ({BUILD_DIR}): this binary serves a 'not built' page; run `just build` first");
    }
}
