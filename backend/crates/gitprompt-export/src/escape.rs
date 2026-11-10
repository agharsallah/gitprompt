//! The one HTML/XML escape every renderer uses (the HTML page and the SVG charts).

use std::borrow::Cow;

/// `text` safe as element content and inside a double- or single-quoted attribute: `&`, `<`, `>`,
/// `"` and `'` become entities, as Python's `html.escape` writes them.
pub(crate) fn html(text: &str) -> Cow<'_, str> {
    if !text.contains(['&', '<', '>', '"', '\'']) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 16);
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}
