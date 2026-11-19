//! What the tray reads: the status, and the daemon's [`Glance`](gitprompt_server::service::Glance)
//! when this app runs it, made into a [`TrayMenu`]. The desktop app calls it on each change the bus
//! reports (coalesced, [`crate::Coalesce`]); nothing polls.

use gitprompt_core::preferences::PreferenceFile;

use crate::state::GitPrompt;
use crate::tray_menu::{TrayMenu, tray_menu};

impl GitPrompt {
    /// The tray's menu now. A read that fails is logged and the menu shows the status alone.
    pub async fn tray_menu(&self) -> TrayMenu {
        let glance = match self.service() {
            Ok(service) => service.glance().await.inspect_err(|e| tracing::warn!(error = %e, "the tray could not read today")).ok(),
            Err(_) => None,
        };
        let settings = self.settings();
        let badge = PreferenceFile::new(&settings.home, settings.env.clone()).effective().leak_badge;
        tray_menu(&self.status(), glance.as_ref(), badge)
    }
}
