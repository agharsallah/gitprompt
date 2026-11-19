//! The weekly recap in the daemon: once at start (a recap due while the daemon was down is sent
//! then, once), then one wait until the next instant it is due ([`Service::until_next_recap`],
//! from the app's clock), and the recap on the bus like any notification ([`Bus::notify`]). The
//! store remembers each week handled, so a restart never sends it twice.

use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::bus::Bus;
use crate::service::Service;

/// The longest single wait before the clock is read again: a timer does not count the time a
/// laptop sleeps, and a changed day or hour applies by then.
pub const CHECK_EVERY: Duration = Duration::from_secs(15 * 60);

/// Send the recap owed now, if any (logged when it cannot be read).
pub async fn send_due(service: &Service) {
    match service.due_recap().await {
        Ok(Some(recap)) => send(service.bus(), recap),
        Ok(None) => {}
        Err(e) => tracing::warn!(error = %e, "could not make the weekly recap"),
    }
}

fn send(bus: &Bus, recap: gitprompt_insights::notify::Notification) {
    tracing::info!(title = %recap.title, "weekly recap");
    bus.notify(recap);
}

/// Wait until the next recap is due (at most `every` at a time), send it, and again, until `stop`.
pub async fn run(service: Service, every: Duration, stop: CancellationToken) {
    loop {
        let wait = Duration::try_from(service.until_next_recap()).unwrap_or(Duration::ZERO).min(every);
        tokio::select! {
            () = stop.cancelled() => return,
            () = tokio::time::sleep(wait) => send_due(&service).await,
        }
    }
}
