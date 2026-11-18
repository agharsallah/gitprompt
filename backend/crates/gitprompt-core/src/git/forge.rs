//! Where a commit lives on the web: its page on GitHub, GitLab or Bitbucket, read from the remote's
//! URL alone (no network, no git run), so a hash from anywhere can open on its forge.

/// The forges whose commit pages are known, by host.
#[derive(Clone, Copy)]
enum Forge {
    GitHub,
    GitLab,
    Bitbucket,
}

impl Forge {
    fn of(host: &str) -> Option<Forge> {
        match host.to_ascii_lowercase().as_str() {
            "github.com" => Some(Forge::GitHub),
            "gitlab.com" => Some(Forge::GitLab),
            "bitbucket.org" => Some(Forge::Bitbucket),
            _ => None,
        }
    }

    /// The commit page under a repository's page.
    fn commit_path(self) -> &'static str {
        match self {
            Forge::GitHub => "commit",
            Forge::GitLab => "-/commit",
            Forge::Bitbucket => "commits",
        }
    }

    /// GitLab nests groups (`group/sub/name`); the others are `owner/name`.
    fn takes(self, segments: usize) -> bool {
        match self {
            Forge::GitLab => segments >= 2,
            Forge::GitHub | Forge::Bitbucket => segments == 2,
        }
    }
}

/// The page of commit `hash` on the forge that hosts `remote` (as `git remote get-url` prints it):
/// `https://<host>/<repo>/commit/<hash>` (GitLab `/-/commit/`, Bitbucket `/commits/`). Takes the
/// https and ssh forms (`ssh://git@host[:port]/repo`, `git@host:repo`), with or without `.git`.
/// `None` for another host, a remote without an `owner/name` path, or a hash that is not hex.
pub fn commit_url(remote: &str, hash: &str) -> Option<String> {
    if hash.is_empty() || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let (host, path) = host_and_path(remote.trim())?;
    let forge = Forge::of(host)?;
    let repo = path.trim_matches('/');
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    let segments: Vec<&str> = repo.split('/').collect();
    if segments.iter().any(|s| s.is_empty()) || !forge.takes(segments.len()) {
        return None;
    }
    Some(format!("https://{}/{repo}/{}/{hash}", host.to_ascii_lowercase(), forge.commit_path()))
}

/// The host (no user, no port) and the path of a remote URL: `scheme://[user@]host[:port]/path`,
/// or scp-like `[user@]host:path`.
fn host_and_path(remote: &str) -> Option<(&str, &str)> {
    let (authority, path) = match remote.split_once("://") {
        Some((_, rest)) => rest.split_once('/')?,
        None => remote.split_once(':').filter(|(authority, _)| !authority.contains('/'))?,
    };
    let host = authority.rsplit('@').next()?;
    let host = host.split(':').next()?;
    (!host.is_empty()).then_some((host, path))
}
