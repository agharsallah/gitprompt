//! `Error::status`: the one place domain errors become HTTP statuses.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, fixtures use small known values, and helpers are crate-private anyway"
)]

use gitprompt_core::Error;
use pretty_assertions::assert_eq;

fn io() -> Error {
    std::io::Error::new(std::io::ErrorKind::NotFound, "gone").into()
}

fn json() -> Error {
    serde_json::from_str::<serde_json::Value>("{oops").unwrap_err().into()
}

#[test]
fn each_error_kind_answers_with_its_http_status() {
    let cases: Vec<(Error, u16)> = vec![
        (Error::Invalid("x".into()), 400),
        (Error::NotFound("x".into()), 404),
        (Error::Conflict("x".into()), 409),
        (Error::Config("x".into()), 409),
        (Error::PolicyBlocked("x".into()), 409),
        (Error::Locked("x".into()), 409),
        (Error::InvalidPreferences("x".into()), 422),
        (Error::Spec("x".into()), 500),
        (Error::Storage("x".into()), 500),
        (io(), 500),
        (json(), 500),
    ];
    let got: Vec<(String, u16)> = cases.iter().map(|(e, _)| (format!("{e:?}"), e.status())).collect();
    let want: Vec<(String, u16)> = cases.iter().map(|(e, s)| (format!("{e:?}"), *s)).collect();
    assert_eq!(got, want);
}

#[test]
fn messages_are_shown_as_given_and_storage_errors_say_so() {
    assert_eq!(Error::Locked("set by GITPROMPT_X".into()).to_string(), "set by GITPROMPT_X");
    assert_eq!(Error::Storage("disk full".into()).to_string(), "storage: disk full");
    assert_eq!(io().to_string(), "gone");
}
