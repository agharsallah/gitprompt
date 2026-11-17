//! `gitprompt find` and `gitprompt resume`: the palette's questions asked from the terminal. Both
//! open the same [`Service`] the dashboard uses, so a match here is a match in ⌘K; this module only
//! asks and prints (nothing here is run: a resume command is printed, never executed).
//!
//! Exit codes: 0 when something matched (or a command was printed), 1 when nothing did or the
//! answer could not be given (no database, an ambiguous name, a tool without a resume command).

mod find;
mod resume;

use std::sync::Arc;

use gitprompt_core::Settings;
use gitprompt_server::{App, AppOptions, FixedClock, Service};

use crate::Io;
use crate::commands::Failure;

pub(crate) use find::find;
pub(crate) use resume::resume;

/// The service over this home's database, its clock the entry point's `now`; `None` (and a note on
/// stderr) before the database exists, which is never created here.
fn open(settings: &Settings, io: &mut Io<'_>) -> Result<Option<Service>, Failure> {
    let db = settings.db_path();
    if !db.exists() {
        writeln!(io.stderr, "No database yet at {}: start GitPrompt and send a prompt first.", db.display())?;
        return Ok(None);
    }
    let options = AppOptions { clock: Arc::new(FixedClock(io.now)), ..AppOptions::default() };
    Ok(Some(App::new(settings.clone(), options)?.service()))
}

/// Run an async service call to its end.
fn block<T>(call: impl Future<Output = T>) -> Result<T, Failure> {
    Ok(tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(call))
}
