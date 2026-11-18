//! Files whose contents are secrets: an agent reading one sends its contents to the model.
//! Each rule is a named row of [`SENSITIVE_FILES`]; adding one is data.

/// How a rule recognizes a path (compared lowercase, with `/` separators).
#[derive(Clone, Copy, Debug)]
pub enum Rule {
    /// The file is named exactly one of these.
    Named(&'static [&'static str]),
    /// The file name ends with one of these extensions (without the dot).
    Extension(&'static [&'static str]),
    /// The path ends with one of these (a file under a known folder).
    Ending(&'static [&'static str]),
    /// `.env` or `.env.<anything>` that is not a template (`.example`, `.sample`, `.template`...).
    Environment,
}

/// (what the file is, how to recognize it), in the order rules are tried.
pub const SENSITIVE_FILES: &[(&str, Rule)] = &[
    ("environment file", Rule::Environment),
    (
        "cloud credentials",
        Rule::Ending(&[".aws/credentials", ".kube/config", ".docker/config.json", "gcloud/application_default_credentials.json"]),
    ),
    ("SSH private key", Rule::Named(&["id_rsa", "id_dsa", "id_ecdsa", "id_ed25519"])),
    ("key or certificate file", Rule::Extension(&["pem", "key", "p12", "pfx", "jks", "keystore", "ppk"])),
    (
        "credentials file",
        Rule::Named(&[
            "credentials",
            "credentials.json",
            ".git-credentials",
            ".netrc",
            ".pgpass",
            ".npmrc",
            ".pypirc",
            ".htpasswd",
            "secrets.json",
            "secrets.yaml",
            "secrets.yml",
            "service-account.json",
        ]),
    ),
];

/// Template endings of an environment file: they hold names, not values.
const ENV_TEMPLATES: &[&str] = &["example", "sample", "template", "dist", "defaults", "schema"];

/// What sensitive file `path` is, if it is one.
pub fn sensitive_file(path: &str) -> Option<&'static str> {
    let path = path.trim().replace('\\', "/").to_ascii_lowercase();
    let name = path.rsplit('/').next().unwrap_or_default();
    SENSITIVE_FILES.iter().find(|(_, rule)| matches(*rule, &path, name)).map(|(what, _)| *what)
}

fn matches(rule: Rule, path: &str, name: &str) -> bool {
    match rule {
        Rule::Named(names) => names.contains(&name),
        Rule::Extension(extensions) => name.rsplit_once('.').is_some_and(|(stem, ext)| !stem.is_empty() && extensions.contains(&ext)),
        Rule::Ending(endings) => endings.iter().any(|ending| path.ends_with(ending)),
        Rule::Environment => {
            name == ".env"
                || name.strip_prefix(".env.").is_some_and(|rest| !ENV_TEMPLATES.iter().any(|t| rest.split('.').any(|part| part == *t)))
        }
    }
}
