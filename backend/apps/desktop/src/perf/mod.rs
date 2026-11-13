//! The desktop app measuring itself, in the performance profile (`GITPROMPT_ENV=performance`, see
//! `.env.performance`; `just quality desktop` runs it). It marks the launch's milestones on the
//! system clock (main, setup, daemon ready, window built, page loaded), runs the page probes of
//! `probes.js` in the real webview ([`scenario`]) and writes everything as JSON. Off in every other
//! profile: nothing here runs and no script is added to the page.

mod bridge;
mod config;
mod scenario;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};
use tauri::webview::{PageLoadEvent, WebviewWindowBuilder};
use tauri::{AppHandle, Manager, Runtime};

use gitprompt_core::{Profile, Settings};

use self::config::PerfConfig;

/// The page probes, run before the dashboard's own scripts.
const PROBES: &str = include_str!("probes.js");

pub(crate) struct Perf {
    config: PerfConfig,
    marks: Mutex<Map<String, Value>>,
    started: AtomicBool,
}

/// Now on the system clock, in milliseconds: what the page's `performance.timeOrigin` uses too.
pub(crate) fn epoch_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

/// The performance run when the profile asks for one, with `main_at` as its first mark; Err when
/// its configuration is wrong.
pub(crate) fn from_settings(settings: Option<&Settings>, main_at: f64) -> Result<Option<Arc<Perf>>, String> {
    let Some(settings) = settings.filter(|s| s.profile == Profile::Performance) else { return Ok(None) };
    let perf = Perf { config: PerfConfig::from_env(&settings.env)?, marks: Mutex::default(), started: AtomicBool::new(false) };
    perf.mark_at("main", main_at);
    Ok(Some(Arc::new(perf)))
}

/// Marks a milestone now, when this is a performance run.
pub(crate) fn mark<R: Runtime>(app: &AppHandle<R>, name: &str) {
    if let Some(perf) = app.try_state::<Arc<Perf>>() {
        perf.mark_at(name, epoch_ms());
    }
}

/// The main window as a performance run wants it: the probes in the page, a private store for a
/// cold launch, and the run started once the dashboard loaded. Unchanged otherwise.
pub(crate) fn configure<'a, R: Runtime, M: Manager<R>>(
    app: &AppHandle<R>,
    builder: WebviewWindowBuilder<'a, R, M>,
) -> WebviewWindowBuilder<'a, R, M> {
    let Some(perf) = app.try_state::<Arc<Perf>>().map(|state| Arc::clone(&state)) else { return builder };
    builder.initialization_script(PROBES).incognito(perf.config.cold).on_page_load(move |window, payload| {
        if payload.event() == PageLoadEvent::Finished && !perf.started.swap(true, Ordering::SeqCst) {
            perf.mark_at("pageLoaded", epoch_ms());
            let perf = Arc::clone(&perf);
            std::thread::spawn(move || scenario::run(&window, &perf));
        }
    })
}

impl Perf {
    fn mark_at(&self, name: &str, at: f64) {
        if let Ok(mut marks) = self.marks.lock() {
            marks.insert(name.to_owned(), Value::from(at));
        }
    }

    fn marks(&self) -> Value {
        Value::Object(self.marks.lock().map(|marks| marks.clone()).unwrap_or_default())
    }

    fn write(&self, report: &Value) -> std::io::Result<()> {
        std::fs::write(&self.config.out, serde_json::to_vec_pretty(report)?)
    }
}
