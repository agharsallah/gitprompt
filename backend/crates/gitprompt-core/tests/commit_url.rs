//! A commit's page on its forge, from the remote URL alone.

use gitprompt_core::git::commit_url;
use pretty_assertions::assert_eq;

const HASH: &str = "a1b2c3d";

#[test]
fn each_forge_in_both_url_forms_with_or_without_dot_git() {
    let github = Some("https://github.com/owner/app/commit/a1b2c3d");
    let gitlab = Some("https://gitlab.com/group/sub/app/-/commit/a1b2c3d");
    let bitbucket = Some("https://bitbucket.org/owner/app/commits/a1b2c3d");
    let cases = [
        ("https://github.com/owner/app.git", github),
        ("https://github.com/owner/app", github),
        ("https://github.com/owner/app/", github),
        ("git@github.com:owner/app.git", github),
        ("ssh://git@github.com/owner/app.git", github),
        ("ssh://git@GitHub.com:22/owner/app", github),
        ("https://gitlab.com/group/sub/app.git", gitlab),
        ("git@gitlab.com:group/sub/app.git", gitlab),
        ("https://bitbucket.org/owner/app.git", bitbucket),
        ("git@bitbucket.org:owner/app", bitbucket),
        ("https://user@bitbucket.org/owner/app.git", bitbucket),
    ];
    for (remote, url) in cases {
        assert_eq!(commit_url(remote, HASH).as_deref(), url, "{remote}");
    }
}

#[test]
fn an_unknown_host_or_shape_gives_none() {
    for remote in [
        "https://git.example.com/owner/app.git",
        "git@example.com:owner/app.git",
        "/srv/git/app.git",
        "../app",
        "https://github.com/owner",
        "https://github.com/owner/app/extra",
        "git@github.com:owner//app",
        "",
    ] {
        assert_eq!(commit_url(remote, HASH), None, "{remote}");
    }
}

#[test]
fn a_hash_that_is_not_hex_gives_none() {
    for hash in ["", "HEAD", "a1b2c3d/../x", "zz"] {
        assert_eq!(commit_url("https://github.com/owner/app", hash), None, "{hash}");
    }
}
