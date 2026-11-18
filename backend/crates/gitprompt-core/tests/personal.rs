//! Personal data and sensitive files: for each kind, what it finds and the near misses it leaves.
//! Every value here is synthetic; card and account numbers are built to pass their checksums.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, and helpers are crate-private anyway"
)]

use gitprompt_core::leak::masked;
use gitprompt_core::personal::{DETECTORS, PRIVATE_TERM, find_personal, find_terms, sensitive_file};
use pretty_assertions::assert_eq;

/// (kind, text) → the values found, masked.
fn found(text: &str) -> Vec<(&'static str, String)> {
    find_personal(text).into_iter().map(|f| (f.kind, masked(f.mask, f.value))).collect()
}

/// One kind's cases: its name, texts holding one value (with the value masked), and near misses.
type Case = (&'static str, &'static [(&'static str, &'static str)], &'static [&'static str]);

/// Each kind: texts that hold one, and near misses that must find nothing.
const TABLE: &[Case] = &[
    (
        "email address",
        &[("write to marie.curie@dupont.io today", "m…@dupont.io"), ("(<Bob.Smith+git@mail.co.uk>)", "B…@mail.co.uk")],
        &[
            "git@github.com:acme/app.git",
            "logo@2x.png and icon@3x.svg",
            "npm i @types/node lodash@4.17.21",
            "user@example.com or test@demo.test",
            "noreply@github.com, 1234+octo@users.noreply.github.com",
            "Author: Jane Doe <jane@acme.io>",
            "Co-authored-by: Bob <bob@acme.io>",
            "  \"author\": \"Jane <jane@acme.io>\",",
            "DATABASE_URL=postgresql://u:pw@ep-x.neon.tech/db",
            "SENTRY_DSN=https://k3y@o0.ingest.sentry.io/1",
            "please contact support@vendor.io",
            "JIRA_EMAIL=you@company.com",
            "\"user.email\": \"me@corp.io\",",
            "\\n@router.get(\"/problems\")",
            "240@app.command() and getty@tty1.service",
            "invoiceEmail: 'a@b.ch'",
            "Copyright 2020 Jane Doe <jane@gmail.com>",
            "`ana@google.com ben@google.com cy@google.com`",
            "Department, University  *  Seattle  *  wobbrock@uw.edu",
            "write me@mail.com, or jo@acme.io",
            "https://hackers.pub/@someone@fosstodon.org",
            "Lisa, Ph.D.\n *  Suceava, Romania\n *  radu@usm.ro",
            "or write to corporate@oreilly.com",
            "--cred \"account_mail@mail.com:password01\"",
        ],
    ),
    (
        "phone number",
        &[("call +49 30 1234567 tomorrow", "+49…567"), ("tel: +1 (415) 555-0132", "+14…132")],
        &[
            "UTC+01:00",
            "version +1.2.3",
            "on +2026-11-18",
            "x+12345",
            "+1234567890123456789",
            "Sun May 11 07:12:54 +0000 2014",
            "m=+0.001234567",
        ],
    ),
    (
        "bank account (IBAN)",
        &[("pay DE44 5001 0517 5407 3249 31 now", "DE44…4931"), ("IBAN: GB33BUKB20201555555555", "GB33…5555")],
        &["DE89370400440532013000", "DE44500105175407324932", "XX44500105175407324931", "SHA256SUMS1234ABCD"],
    ),
    (
        "card number",
        &[("card 4539 5787 6362 1486 exp", "…1486"), ("paid with card 5425233430109903", "…9903")],
        &[
            "4242 4242 4242 4242",
            "card 4539578763621487",
            "1234567890123456",
            "2026-11-18 10:00:00 1234",
            "0000000000000000",
            "{\"id\": 5425233430109903}",
            "1 3 1 12 1 9 1 1 1 1 3 6 1 10 3 4 5 11 2",
        ],
    ),
];

#[test]
fn the_table_covers_every_detector_in_order() {
    let names: Vec<&str> = TABLE.iter().map(|(kind, ..)| *kind).collect();
    assert_eq!(names, DETECTORS.iter().map(|d| d.name).collect::<Vec<_>>());
}

#[test]
fn each_kind_finds_its_values_masked() {
    for (kind, hits, _) in TABLE {
        for (text, mask) in *hits {
            assert_eq!(found(text), [(*kind, (*mask).to_owned())], "{text}");
        }
    }
}

#[test]
fn each_kind_leaves_its_near_misses() {
    for (_, _, misses) in TABLE {
        for text in *misses {
            assert_eq!(found(text), [], "{text}");
        }
    }
}

/// Text coding sessions are full of: hashes, ids, versions, timestamps, lock files. None of it is
/// personal, and none of it may be reported.
const NO_FALSE_ALARM: &str = r#"
commit 9fceb02d0ae598e95dc970b74767f19372d61af8
Merge: 1a2b3c4 5d6e7f8
Date:   Tue Nov 18 10:42:11 2026 +0100
id: 3f2504e0-4f89-11d3-9a0c-0305e82c3301 and 550e8400-e29b-41d4-a716-446655440000
version = "1.2.3" ; rust 1.99.0 ; node v22.11.0 ; build 20261118.4
sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
"resolved": "https://registry.npmjs.org/lodash/-/lodash-4.17.21.tgz",
"integrity": "sha512-v2kDEe57lecTulaDIuNTPy3Ry4gLGJ6Z1O3vE1krgXZNrsQ+LFTGHVxVjcXPs17LhbZVGedAJv8XZ1tvj5FvSg==",
checksum = "a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192"
at 2026-11-18T10:42:11.123456+00:00 took 1234567 µs, port 54321, pid 98765
let ids = [1700000000000, 1700000000001, 9007199254740991];
@@ -12,7 +12,9 @@ fn main() {
+    let total = 1234567890123;
"#;

#[test]
fn hashes_ids_versions_and_lock_files_raise_no_alarm() {
    assert_eq!(found(NO_FALSE_ALARM), []);
}

#[test]
fn your_own_words_are_found_as_whole_words_in_any_case() {
    let terms = vec!["Project Falcon".to_owned(), "acme-internal".to_owned(), "  ".to_owned()];
    let text = "the project falcon roadmap, see acme-internal.wiki; not falconry";
    let values: Vec<(&str, &str)> = find_terms(text, &terms).into_iter().map(|f| (f.kind, f.value)).collect();
    assert_eq!(values, [(PRIVATE_TERM, "project falcon"), (PRIVATE_TERM, "acme-internal")]);
    assert_eq!(find_terms("falconry", &["falcon".to_owned()]), []);
}

#[test]
fn sensitive_files_are_named_and_templates_are_not() {
    let cases = [
        ("/repo/.env", Some("environment file")),
        ("./.env.production.local", Some("environment file")),
        ("/repo/.env.example", None),
        ("config/.env.sample", None),
        ("/home/me/.ssh/id_ed25519", Some("SSH private key")),
        ("/home/me/.ssh/id_ed25519.pub", None),
        ("certs/server.pem", Some("key or certificate file")),
        ("C:\\Users\\me\\.aws\\credentials", Some("cloud credentials")),
        ("~/.npmrc", Some("credentials file")),
        ("src/keyboard.rs", None),
        ("docs/environment.md", None),
    ];
    for (path, kind) in cases {
        assert_eq!(sensitive_file(path), kind, "{path}");
    }
}
