//! What a possible leak is, wherever it was found ([`crate::secrets`], [`crate::personal`]): its
//! kind, the matched text, and how to show it without showing it. Plus the checks that keep the
//! false alarms out: placeholders and values too regular to be a real key.

/// One value that looks like a leak. `value` borrows the text it was found in; only its
/// [`masked`] form may be stored or shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found<'t> {
    pub kind: &'static str,
    pub value: &'t str,
    pub mask: Mask,
    pub scope: Scope,
}

/// Where a kind of value counts as a leak.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Wherever it is: a key is a key, even in a file the agent read.
    Anywhere,
    /// In what was sent or asked (a prompt, an answer, a tool's input), not in what a tool read
    /// back: a phone number on a web page or in a README is published, not leaked.
    Sent,
}

/// How a kind of value is shown masked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mask {
    /// `sk-…a91f`: the first 3 and last 4 characters of a long value, the first one of a short one.
    Token,
    /// `j…@example.org`: the first character of the name, then the domain.
    Email,
    /// `+49…567`: the country code's start and the last 3 digits.
    Phone,
    /// `DE89…3000`: the country, check digits and the last 4.
    Account,
    /// `…4242`: the last 4 digits.
    Card,
    /// Shown as is (a file name is where a secret is, not the secret).
    Plain,
    /// Not shown at all (a private key block): `hidden`.
    Hidden,
}

/// Shortest token shown by its head and tail; a shorter one shows only its first character.
const TOKEN_ENDS_FROM: usize = 12;

/// The value as it may be shown.
pub fn masked(mask: Mask, value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let head = |n: usize| chars.iter().take(n).collect::<String>();
    let tail = |n: usize| chars.iter().skip(chars.len().saturating_sub(n)).collect::<String>();
    match mask {
        Mask::Token if chars.len() >= TOKEN_ENDS_FROM => format!("{}…{}", head(3), tail(4)),
        Mask::Token => format!("{}…", head(1)),
        Mask::Email => match value.split_once('@') {
            Some((name, domain)) => format!("{}…@{domain}", name.chars().next().unwrap_or('?')),
            None => format!("{}…", head(1)),
        },
        Mask::Phone => {
            let digits: String = value.chars().filter(char::is_ascii_digit).collect();
            let last = digits.get(digits.len().saturating_sub(3)..).unwrap_or_default();
            format!("+{}…{last}", digits.get(..2).unwrap_or_default())
        }
        Mask::Account => {
            let compact: String = value.chars().filter(|c| !c.is_whitespace()).collect();
            let tail4 = compact.get(compact.len().saturating_sub(4)..).unwrap_or_default();
            format!("{}…{tail4}", compact.get(..4).unwrap_or_default())
        }
        Mask::Card => {
            let digits: String = value.chars().filter(char::is_ascii_digit).collect();
            format!("…{}", digits.get(digits.len().saturating_sub(4)..).unwrap_or_default())
        }
        Mask::Plain => value.to_owned(),
        Mask::Hidden => "hidden".to_owned(),
    }
}

/// Shannon entropy in bits per character: about 4 to 6 for a random key, under 3 for words,
/// repeated characters and most placeholders.
pub fn entropy(text: &str) -> f64 {
    let mut counts = std::collections::HashMap::<char, usize>::new();
    let mut total = 0usize;
    for c in text.chars() {
        *counts.entry(c).or_default() += 1;
        total += 1;
    }
    if total == 0 {
        return 0.0;
    }
    #[expect(clippy::cast_precision_loss, reason = "character counts are small")]
    let total = total as f64;
    counts.values().fold(0.0, |sum, &n| {
        #[expect(clippy::cast_precision_loss, reason = "character counts are small")]
        let p = n as f64 / total;
        sum - p * p.log2()
    })
}

/// Words that mark a value as an example, not a real one.
const PLACEHOLDER_WORDS: &[&str] = &[
    "example",
    "placeholder",
    "changeme",
    "change_me",
    "your_",
    "your-",
    "yourkey",
    "redacted",
    "dummy",
    "sample",
    "secret",
    "password",
    "passw",
    "token",
    "hunter2",
    "foobar",
    "lorem",
    "xxxx",
    "****",
    "...",
    "<",
    "${",
    "{{",
];

/// A value someone wrote as a stand-in: an example word, a template, or a run of one character.
pub fn is_placeholder(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if PLACEHOLDER_WORDS.iter().any(|word| lower.contains(word)) {
        return true;
    }
    let mut chars = lower.chars().filter(char::is_ascii_alphanumeric);
    chars.next().is_some_and(|first| chars.all(|c| c == first))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_masked_by_kind() {
        assert_eq!(masked(Mask::Token, "sk-proj-abcdefghijklmnopa91f"), "sk-…a91f");
        assert_eq!(masked(Mask::Token, "hunter2"), "h…");
        assert_eq!(masked(Mask::Email, "jane.doe@acme.io"), "j…@acme.io");
        assert_eq!(masked(Mask::Phone, "+49 30 1234567"), "+49…567");
        assert_eq!(masked(Mask::Account, "DE89 3704 0044 0532 0130 00"), "DE89…3000");
        assert_eq!(masked(Mask::Card, "4111 1111 1111 1111"), "…1111");
        assert_eq!(masked(Mask::Plain, ".env"), ".env");
    }

    #[test]
    fn random_keys_have_high_entropy_and_words_low() {
        assert!(entropy("q8Zt2Lw9XkP4vR7mN3cB") > 4.0);
        assert!(entropy("aaaaaaaaaaaaaaaa") < 0.1);
        assert!(entropy("passwordpassword") < 3.0);
    }

    #[test]
    fn placeholders_are_recognized() {
        for value in ["your_api_key_here", "<TOKEN>", "${API_KEY}", "xxxxxxxxxxxx", "sk-EXAMPLE123", "000000000000"] {
            assert!(is_placeholder(value), "{value}");
        }
        assert!(!is_placeholder("q8Zt2Lw9XkP4vR7mN3cB"));
    }
}
