//! The notifier: after each capture notice on the [`Bus`], it gathers what may deserve a
//! notification ([`Service::check_notifications`]), lets
//! [`gitprompt_insights::notify::decide`] choose with "now" from the app's clock, and sends each
//! notification back on the bus (the dashboard's live stream and the desktop shell read it). It
//! runs in the daemon, so it works with the window closed.

use tokio_util::sync::CancellationToken;

use crate::bus::{Bus, Subscription};
use crate::service::{Service, Watch};

#[derive(Debug)]
pub struct Notifier {
    service: Service,
    notices: Subscription,
    watch: Watch,
}

impl Notifier {
    /// Subscribed to the bus and watching from now: nothing captured before is notified. `None`
    /// (logged) when the store cannot be read; the daemon then runs without notifications.
    pub async fn start(service: Service) -> Option<Notifier> {
        let notices = service.bus().subscribe();
        match service.watch().await {
            Ok(watch) => Some(Notifier { service, notices, watch }),
            Err(e) => {
                tracing::warn!(error = %e, "notifications are off: could not read the store");
                None
            }
        }
    }

    /// One check per notice (a burst of notices is one check) until `stop` or the bus is gone.
    pub async fn run(mut self, stop: CancellationToken) {
        loop {
            tokio::select! {
                () = stop.cancelled() => return,
                notice = self.notices.next() => {
                    if notice.is_none() {
                        return;
                    }
                    while self.notices.try_next().is_some() {}
                    self.check().await;
                }
            }
        }
    }

    async fn check(&mut self) {
        let watch = std::mem::take(&mut self.watch);
        match self.service.check_notifications(watch).await {
            Ok((watch, found)) => {
                self.watch = watch;
                match found {
                    Ok(found) => send(self.service.bus(), found),
                    Err(e) => tracing::warn!(error = %e, "could not check for notifications"),
                }
            }
            Err(e) => self.restart(&e.to_string()).await,
        }
    }

    /// The watch was lost with a failed check: watch again from now.
    async fn restart(&mut self, why: &str) {
        tracing::warn!(error = %why, "the notification check failed; watching again from now");
        if let Ok(watch) = self.service.watch().await {
            self.watch = watch;
        }
    }
}

fn send(bus: &Bus, found: Vec<gitprompt_insights::notify::Notification>) {
    for notification in found {
        tracing::info!(kind = ?notification.kind, title = %notification.title, "notification");
        bus.notify(notification);
    }
}

/// Run `notifier` (if it started) until `stop`.
pub async fn run(notifier: Option<Notifier>, stop: CancellationToken) {
    if let Some(notifier) = notifier {
        notifier.run(stop).await;
    }
}
