//! [`CommandError`]: what every command returns on failure, serialized as `{status, detail}` with
//! the HTTP status the same failure has on the API.

use std::fmt;

use serde::{Deserialize, Serialize};

use gitprompt_server::ApiError;

/// `{"status": 409, "detail": "..."}`: the frontend handles a command failure exactly like an API
/// failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandError {
    pub status: u16,
    pub detail: String,
}

impl CommandError {
    pub fn new(status: u16, detail: impl Into<String>) -> Self {
        CommandError { status, detail: detail.into() }
    }

    /// 400: an argument that doesn't make sense.
    pub fn invalid(detail: impl Into<String>) -> Self {
        Self::new(400, detail)
    }

    /// 404.
    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(404, detail)
    }

    /// 500.
    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(500, detail)
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.detail, self.status)
    }
}

impl std::error::Error for CommandError {}

impl From<ApiError> for CommandError {
    fn from(error: ApiError) -> Self {
        let body = error.body();
        CommandError { status: body.status, detail: body.detail }
    }
}

impl From<std::io::Error> for CommandError {
    fn from(error: std::io::Error) -> Self {
        Self::internal(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn serializes_as_status_and_detail() {
        let value = serde_json::to_value(CommandError::new(409, "nope")).unwrap();
        assert_eq!(value, json!({"status": 409, "detail": "nope"}));
    }

    #[test]
    fn api_errors_keep_their_http_status() {
        let cases = [
            (ApiError::Domain(gitprompt_core::Error::NotFound("prompt not found".into())), 404, "prompt not found"),
            (ApiError::Domain(gitprompt_core::Error::Conflict("disabled".into())), 409, "disabled"),
            (ApiError::Unprocessable("limit".into()), 422, "limit"),
            (ApiError::Internal("boom".into()), 500, "boom"),
        ];
        for (error, status, detail) in cases {
            assert_eq!(CommandError::from(error), CommandError::new(status, detail));
        }
    }

    #[test]
    fn io_errors_are_internal() {
        let error = CommandError::from(std::io::Error::other("disk full"));
        assert_eq!(error, CommandError::new(500, "disk full"));
    }

    #[test]
    fn round_trips() {
        let error = CommandError::invalid("bad");
        let back: CommandError = serde_json::from_value(serde_json::to_value(&error).unwrap()).unwrap();
        assert_eq!(back, error);
    }
}
