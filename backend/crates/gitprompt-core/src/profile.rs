//! What a GitPrompt process runs for, from `GITPROMPT_ENV`: `development` (`just dev`),
//! `production` (an installed app or `gitprompt serve`; the default when unset) or `performance`
//! (the desktop app measuring itself for `just quality desktop`). The repository's `.env.<profile>`
//! files set it for the `just` recipes; the binaries never read `.env` files themselves.

use std::fmt;

use crate::{Env, Error, Result};

/// The variable naming the profile.
pub const PROFILE_ENV: &str = "GITPROMPT_ENV";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Profile {
    Development,
    #[default]
    Production,
    Performance,
}

impl Profile {
    /// The profile `GITPROMPT_ENV` names (`dev` and `prod` too), production when unset; an
    /// `Error::Invalid` naming the variable for anything else.
    pub fn from_env(env: &Env) -> Result<Self> {
        match env.get(PROFILE_ENV) {
            None => Ok(Profile::Production),
            Some(text) => Self::parse(text)
                .ok_or_else(|| Error::Invalid(format!("{PROFILE_ENV}={text}: expected development, production or performance"))),
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "development" | "dev" => Some(Profile::Development),
            "production" | "prod" => Some(Profile::Production),
            "performance" | "perf" => Some(Profile::Performance),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Profile::Development => "development",
            Profile::Production => "production",
            Profile::Performance => "performance",
        }
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(value: Option<&str>) -> Result<Profile> {
        Profile::from_env(&Env::from_pairs(value.map(|v| (PROFILE_ENV, v))))
    }

    #[test]
    fn unset_is_production() {
        assert_eq!(profile(None).unwrap(), Profile::Production);
        assert_eq!(profile(Some("")).unwrap(), Profile::Production);
    }

    #[test]
    fn names_and_short_names_in_any_case() {
        assert_eq!(profile(Some("development")).unwrap(), Profile::Development);
        assert_eq!(profile(Some("DEV")).unwrap(), Profile::Development);
        assert_eq!(profile(Some("prod")).unwrap(), Profile::Production);
        assert_eq!(profile(Some(" performance ")).unwrap(), Profile::Performance);
        assert_eq!(profile(Some("perf")).unwrap(), Profile::Performance);
    }

    #[test]
    fn anything_else_names_the_variable() {
        let error = profile(Some("staging")).unwrap_err().to_string();
        assert!(error.contains("GITPROMPT_ENV=staging"), "{error}");
    }

    #[test]
    fn displays_its_full_name() {
        assert_eq!(Profile::Performance.to_string(), "performance");
    }
}
