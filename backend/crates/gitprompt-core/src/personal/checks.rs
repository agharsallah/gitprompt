//! The checks that tell a real email, phone number, IBAN or card number from a look-alike: a
//! checksum where the format has one, the well-known example values, and the lines that only
//! quote public metadata. Each also checks the edges of its match (the patterns have none).

use super::Context;

/// A letter, digit or `_`.
fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Domains reserved for examples and tests (RFC 2606, RFC 6761), and other addresses that are
/// never someone's: no-reply senders and GitHub's private commit addresses.
const EXAMPLE_DOMAINS: &[&str] = &[
    "example.com",
    "example.org",
    "example.net",
    "users.noreply.github.com",
    "localhost",
    "company.com",
    "yourcompany.com",
    "domain.com",
    "yourdomain.com",
    "email.com",
    "site.com",
    "test.com",
    "foo.com",
    "bar.com",
];
const EXAMPLE_TLDS: &[&str] = &["example", "test", "invalid", "localhost", "local", "internal"];
const NO_REPLY: &[&str] = &["noreply", "no-reply", "donotreply", "do-not-reply"];
/// Names that stand for "someone" in documentation and tests.
const PLACEHOLDER_NAMES: &[&str] = &[
    "me",
    "you",
    "user",
    "username",
    "name",
    "someone",
    "email",
    "mail",
    "test",
    "your.name",
    "firstname.lastname",
    "first.last",
    "john.doe",
    "jane.doe",
    "john",
    "jane",
    "foo",
    "bar",
];
/// Company names that only exist in examples, as the first part of a domain.
const EXAMPLE_COMPANIES: &[&str] = &["acme", "example", "test", "foo", "bar", "contoso", "fabrikam"];
/// Addresses on one line from which it is a list (paper authors, a directory, a credits file).
const LIST_ADDRESSES: usize = 3;

/// Addresses of a role, published to be written to: not a person's.
const ROLES: &[&str] = &[
    "support",
    "info",
    "contact",
    "hello",
    "help",
    "sales",
    "team",
    "feedback",
    "privacy",
    "security",
    "abuse",
    "postmaster",
    "webmaster",
    "press",
    "careers",
    "jobs",
    "billing",
    "legal",
    "corporate",
    "office",
    "admissions",
];
/// Words of an academic or publishing context near an address: it is published with the work.
const SCHOLARLY_WORDS: &[&str] = &["university", "universit", "ph.d", "department", "institute", "laborator", "<sup>"];

/// What follows `@` in file names (`logo@2x.png`) and package specs, never a mail domain.
const FILE_ENDINGS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "svg", "webp", "ico", "js", "mjs", "cjs", "ts", "tsx", "jsx", "css", "scss", "map", "json", "py", "rs",
    "go", "md", "html", "yaml", "yml", "toml", "lock", "txt", "sh", "rb", "java", "kt", "swift", "vue", "svelte",
];

/// Lines that quote commit or package metadata: public addresses, not something you leaked.
const METADATA_LINES: &[&str] = &[
    "author:",
    "author ",
    "committer",
    "co-authored-by",
    "signed-off-by",
    "reviewed-by",
    "acked-by",
    "\"author\"",
    "maintainer",
    "\"user.email\"",
    "copyright",
];

/// Top-level domains beyond the two-letter country codes that addresses commonly use. A name after
/// `@` ending in anything else is code (`@router.get`, `getty@tty1.service`), not a mail domain.
const MAIL_TLDS: &[&str] = &[
    "com",
    "net",
    "org",
    "edu",
    "gov",
    "mil",
    "int",
    "info",
    "biz",
    "name",
    "pro",
    "app",
    "dev",
    "tech",
    "cloud",
    "email",
    "online",
    "site",
    "xyz",
    "company",
    "agency",
    "digital",
    "studio",
    "design",
    "media",
    "group",
    "global",
    "solutions",
    "consulting",
    "systems",
    "software",
    "network",
    "services",
    "world",
    "life",
    "live",
    "health",
    "law",
    "bank",
    "finance",
    "capital",
    "ventures",
    "partners",
    "academy",
    "school",
    "university",
    "museum",
    "travel",
    "shop",
    "store",
];

/// A real person's address: not an example, no-reply or role address, not `git@host` (an SSH
/// remote), not a file name, not the credentials of a URL (`scheme://user:pass@host`), and not on
/// a line of commit, package or telemetry metadata.
pub(super) fn email(context: &Context<'_>) -> bool {
    // Inside a word, after `:` or `/` (a URL's credentials or user, a DSN) or an escape (`\n@app`).
    // ...or after another `@` (a fediverse handle, `@user@host`).
    if context.before.is_some_and(|c| word(c) || matches!(c, '.' | '%' | '+' | '-' | ':' | '/' | '\\' | '@')) {
        return false;
    }
    let Some((name, domain)) = context.value.rsplit_once('@') else { return false };
    let line = context.line.to_ascii_lowercase();
    let nearby = context.nearby.to_ascii_lowercase();
    let listed = line.matches('@').count() >= LIST_ADDRESSES || SCHOLARLY_WORDS.iter().any(|w| nearby.contains(w));
    !listed
        && !not_a_person(&name.to_ascii_lowercase(), &domain.to_ascii_lowercase())
        && !METADATA_LINES.iter().any(|marker| line.contains(marker))
}

/// An address no one owns or anyone may publish: examples, no-reply and role names, SSH remotes,
/// file names.
fn not_a_person(name: &str, domain: &str) -> bool {
    let tld = domain.rsplit('.').next().unwrap_or_default();
    let label = domain.split('.').next().unwrap_or_default();
    let mail_tld = tld.len() == 2 || MAIL_TLDS.contains(&tld);
    let academic = tld == "edu" || domain.contains(".ac.") || domain.contains("univ") || domain.starts_with("uni-");
    !mail_tld
        || academic
        || label.chars().count() < 2
        || EXAMPLE_COMPANIES.contains(&label)
        || PLACEHOLDER_NAMES.contains(&name)
        || name.split(['.', '_', '-']).any(|part| part == "mail" || part == "email")
        || name.chars().all(|c| c.is_ascii_digit())
        || name == "git"
        || NO_REPLY.iter().any(|word| name.contains(word))
        || ROLES.contains(&name)
        || EXAMPLE_DOMAINS.iter().any(|d| domain == *d || domain.ends_with(&format!(".{d}")))
        || EXAMPLE_TLDS.contains(&tld)
        || FILE_ENDINGS.contains(&tld)
}

/// An international number (E.164: 8 to 15 digits after `+`), not a date written after a `+`.
pub(super) fn phone(context: &Context<'_>) -> bool {
    let value = context.value;
    if context.before.is_some_and(|c| word(c) || c == '+') || context.after.is_some_and(word) {
        return false;
    }
    let digits = value.chars().filter(char::is_ascii_digit).count();
    // No country code starts with 0: `+0000 2014` is a time zone and a year.
    (8..=15).contains(&digits) && !value.starts_with("+0") && !is_date(value.trim_start_matches('+')) && !value.contains("  ")
}

/// `yyyy-mm-dd`.
fn is_date(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    parts.len() == 3 && parts.iter().map(|p| p.len()).eq([4, 2, 2]) && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

/// Account lengths by country (ISO 13616 registry), for the countries most likely to appear.
const IBAN_LENGTHS: &[(&str, usize)] = &[
    ("AD", 24),
    ("AE", 23),
    ("AL", 28),
    ("AT", 20),
    ("AZ", 28),
    ("BA", 20),
    ("BE", 16),
    ("BG", 22),
    ("BH", 22),
    ("BR", 29),
    ("BY", 28),
    ("CH", 21),
    ("CR", 22),
    ("CY", 28),
    ("CZ", 24),
    ("DE", 22),
    ("DK", 18),
    ("DO", 28),
    ("EE", 20),
    ("EG", 29),
    ("ES", 24),
    ("FI", 18),
    ("FO", 18),
    ("FR", 27),
    ("GB", 22),
    ("GE", 22),
    ("GI", 23),
    ("GL", 18),
    ("GR", 27),
    ("GT", 28),
    ("HR", 21),
    ("HU", 28),
    ("IE", 22),
    ("IL", 23),
    ("IQ", 23),
    ("IS", 26),
    ("IT", 27),
    ("JO", 30),
    ("KW", 30),
    ("KZ", 20),
    ("LB", 28),
    ("LC", 32),
    ("LI", 21),
    ("LT", 20),
    ("LU", 20),
    ("LV", 21),
    ("LY", 25),
    ("MA", 28),
    ("MC", 27),
    ("MD", 24),
    ("ME", 22),
    ("MK", 19),
    ("MR", 27),
    ("MT", 31),
    ("MU", 30),
    ("NL", 18),
    ("NO", 15),
    ("PK", 24),
    ("PL", 28),
    ("PS", 29),
    ("PT", 25),
    ("QA", 29),
    ("RO", 24),
    ("RS", 22),
    ("SA", 24),
    ("SC", 31),
    ("SD", 18),
    ("SE", 24),
    ("SI", 19),
    ("SK", 24),
    ("SM", 27),
    ("ST", 25),
    ("SV", 28),
    ("TL", 23),
    ("TN", 24),
    ("TR", 26),
    ("UA", 29),
    ("VA", 22),
    ("VG", 24),
    ("XK", 20),
];

/// Account numbers every tutorial uses.
const EXAMPLE_IBANS: &[&str] =
    &["DE89370400440532013000", "GB82WEST12345698765432", "GB29NWBK60161331926819", "FR1420041010050500013M02606"];

/// A valid IBAN: its country's length and the ISO 7064 mod-97 check.
pub(super) fn iban(context: &Context<'_>) -> bool {
    let value = context.value;
    let compact: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    let length = IBAN_LENGTHS.iter().find(|(country, _)| compact.starts_with(country)).map(|&(_, n)| n);
    length == Some(compact.len()) && !EXAMPLE_IBANS.contains(&compact.as_str()) && mod97(&compact) == Some(1)
}

/// The IBAN remainder: the first four characters moved to the end, letters as 10..35.
fn mod97(compact: &str) -> Option<u32> {
    let (head, rest) = (compact.get(..4)?, compact.get(4..)?);
    rest.chars().chain(head.chars()).try_fold(0u32, |acc, c| {
        let value = c.to_digit(36)?;
        Some(if value < 10 { (acc * 10 + value) % 97 } else { (acc * 100 + value) % 97 })
    })
}

/// The numbers payment providers publish for tests.
const TEST_CARDS: &[&str] = &[
    "4242424242424242",
    "4111111111111111",
    "4012888888881881",
    "4000056655665556",
    "5555555555554444",
    "5105105105105100",
    "2223003122003222",
    "378282246310005",
    "371449635398431",
    "6011111111111117",
    "6011000990139424",
    "3530111333300000",
];

/// A card number: an issuer's prefix and length, the Luhn check, and not a published test card.
pub(super) fn card(context: &Context<'_>) -> bool {
    let value = context.value;
    let edge = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit() || c == '-');
    if edge(context.before) || edge(context.after) {
        return false;
    }
    let digits: String = value.chars().filter(char::is_ascii_digit).collect();
    let groups: Vec<usize> = value.split([' ', '-']).map(str::len).collect();
    if groups.len() > 1 && !card_groups(&groups) {
        return false;
    }
    // A long number written in one run is usually an id; then the line has to speak of a card.
    let grouped = groups.len() > 1;
    let line = context.line.to_ascii_lowercase();
    let about_a_card = grouped || CARD_WORDS.iter().any(|word| line.contains(word));
    about_a_card && issuer(&digits) && luhn(&digits) && !TEST_CARDS.contains(&digits.as_str())
}

/// How cards are printed: fours (the last group shorter), or American Express's 4-6-5.
fn card_groups(groups: &[usize]) -> bool {
    let fours = groups.split_last().is_some_and(|(last, rest)| rest.iter().all(|&g| g == 4) && (1..=4).contains(last));
    fours || groups == [4, 6, 5]
}

/// Words near an ungrouped card number.
const CARD_WORDS: &[&str] = &["card", "visa", "mastercard", "amex", "credit", "debit", "payment", "cc_number", "ccnum", "pan"];

/// Visa, Mastercard, American Express, Discover or JCB, by prefix and length.
fn issuer(digits: &str) -> bool {
    let prefix = |n: usize| digits.get(..n).and_then(|p| p.parse::<u32>().ok()).unwrap_or(0);
    match digits.len() {
        13 | 19 => prefix(1) == 4,
        15 => matches!(prefix(2), 34 | 37),
        16 => {
            prefix(1) == 4
                || (51..=55).contains(&prefix(2))
                || (2221..=2720).contains(&prefix(4))
                || prefix(4) == 6011
                || prefix(2) == 65
                || (3528..=3589).contains(&prefix(4))
        }
        _ => false,
    }
}

fn luhn(digits: &str) -> bool {
    let sum: u32 = digits
        .chars()
        .rev()
        .filter_map(|c| c.to_digit(10))
        .enumerate()
        .map(|(at, d)| if at % 2 == 1 { if d * 2 > 9 { d * 2 - 9 } else { d * 2 } } else { d })
        .sum();
    !digits.is_empty() && sum.is_multiple_of(10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums() {
        assert!(luhn("4539578763621486"));
        assert!(!luhn("4539578763621487"));
        assert_eq!(mod97("DE44500105175407324931"), Some(1));
        assert_ne!(mod97("DE44500105175407324932"), Some(1));
    }
}
