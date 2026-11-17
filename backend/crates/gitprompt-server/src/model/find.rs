//! `GET /api/find`: one search over prompts, sessions and projects.

use serde::{Deserialize, Serialize};

use gitprompt_insights::find::{Kind, PER_KIND};

/// The best matches of each kind, grouped: `{prompts, sessions, projects}`, each a list of hits with
/// a snippet and the id the dashboard links to.
pub use gitprompt_insights::find::{FindResults, Hit as FindHit, Target as FindTarget};

/// Query of `GET /api/find`: `q` (empty: the most recent of each kind), `kinds` (`prompts`,
/// `sessions`, `projects`, comma separated or repeated; default all) and `limit` per kind
/// (1..=[`PER_KIND`], default [`PER_KIND`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindQuery {
    pub q: String,
    pub kinds: Vec<Kind>,
    pub limit: usize,
}

impl Default for FindQuery {
    fn default() -> Self {
        FindQuery { q: String::new(), kinds: Kind::ALL.to_vec(), limit: PER_KIND }
    }
}
