//! The prompt indexer the CLI builds, with the same rules as the daemon's: the specs and the words
//! of your own list (`private_terms`), so a `gitprompt reindex` stamps the version the daemon
//! expects and `gitprompt doctor` compares against it.

use gitprompt_core::Settings;
use gitprompt_core::preferences::PreferenceFile;
use gitprompt_insights::Specs;
use gitprompt_insights::indexer::Indexer;

/// The indexer of `specs` with the private terms of the preferences (none when they can't be read).
pub(crate) fn indexer(settings: &Settings, specs: Specs) -> Indexer {
    let preferences = PreferenceFile::new(&settings.home, settings.env.clone()).effective();
    Indexer::new(specs).with_private_terms(&preferences.private_terms)
}
