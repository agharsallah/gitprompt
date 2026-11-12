//! Rust source reduced to the code that ships: comments, string and char contents, and
//! `#[cfg(test)]` items are blanked (newlines kept, so line numbers still match the file).

use std::path::{Path, PathBuf};

/// Every `.rs` file under `dir`, sorted.
pub fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// The file's shipped code.
pub fn read_code(path: &Path) -> String {
    production(&code_only(&std::fs::read_to_string(path).unwrap()))
}

/// Line number (1-based) of byte `offset`.
pub fn line_of(code: &str, offset: usize) -> usize {
    code[..offset].matches('\n').count() + 1
}

/// Comments and the contents of string and char literals replaced by spaces.
fn code_only(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        let next = chars.get(i + 1).copied();
        let end = if c == '/' && next == Some('/') {
            line_comment_end(&chars, i)
        } else if c == '/' && next == Some('*') {
            block_comment_end(&chars, i)
        } else if c == '"' {
            string_end(&chars, i)
        } else if let Some(end) = raw_string_end(&chars, i).or_else(|| char_literal_end(&chars, i)) {
            end
        } else {
            out.push(c);
            i += 1;
            continue;
        };
        out.extend(chars[i..end].iter().map(|&c| if c == '\n' { '\n' } else { ' ' }));
        i = end;
    }
    out
}

fn line_comment_end(chars: &[char], start: usize) -> usize {
    chars[start..].iter().position(|&c| c == '\n').map_or(chars.len(), |n| start + n)
}

/// Block comments nest in Rust.
fn block_comment_end(chars: &[char], start: usize) -> usize {
    let (mut depth, mut i) = (0, start);
    while i < chars.len() {
        match (chars[i], chars.get(i + 1)) {
            ('/', Some('*')) => (depth, i) = (depth + 1, i + 2),
            ('*', Some('/')) => {
                (depth, i) = (depth - 1, i + 2);
                if depth == 0 {
                    return i;
                }
            }
            _ => i += 1,
        }
    }
    chars.len()
}

fn string_end(chars: &[char], start: usize) -> usize {
    let mut i = start + 1;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '"' => return i + 1,
            _ => i += 1,
        }
    }
    chars.len()
}

/// `r"…"`, `r#"…"#`, `br"…"`: the end of the literal if one starts at `start`.
fn raw_string_end(chars: &[char], start: usize) -> Option<usize> {
    let prefix = if chars[start] == 'b' && chars.get(start + 1) == Some(&'r') { 2 } else { 1 };
    if chars[start + prefix - 1] != 'r' || start.checked_sub(1).is_some_and(|p| is_ident(chars[p])) {
        return None;
    }
    let hashes = chars[start + prefix..].iter().take_while(|&&c| c == '#').count();
    let open = start + prefix + hashes;
    if chars.get(open) != Some(&'"') {
        return None;
    }
    let close: Vec<char> = std::iter::once('"').chain(std::iter::repeat_n('#', hashes)).collect();
    let body = open + 1;
    let found = chars[body..].windows(close.len()).position(|w| w == close.as_slice());
    Some(found.map_or(chars.len(), |n| body + n + close.len()))
}

/// `'x'` or `'\n'` (not a lifetime such as `'a`).
fn char_literal_end(chars: &[char], start: usize) -> Option<usize> {
    if chars[start] != '\'' {
        return None;
    }
    if chars.get(start + 1) == Some(&'\\') {
        // Skip the escaped character itself, so `'\''` ends at its last quote.
        let close = chars.get(start + 3..)?.iter().position(|&c| c == '\'')?;
        return Some(start + 3 + close + 1);
    }
    (chars.get(start + 2) == Some(&'\'')).then_some(start + 3)
}

pub fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `code` without its `#[cfg(test)]` items (a `mod tests { … }`, a `use`, a `fn`), or blank when
/// the whole file is `#![cfg(test)]`.
fn production(code: &str) -> String {
    if code.contains("#![cfg(test)]") {
        return blank(code);
    }
    let mut out = code.to_owned();
    let mut from = 0;
    while let Some(found) = out[from..].find("#[cfg(test)]") {
        let start = from + found;
        let end = item_end(out.as_bytes(), start + "#[cfg(test)]".len());
        out.replace_range(start..end, &blank(&out[start..end]));
        from = end;
    }
    out
}

/// Where the item starting at `start` ends: after its first `;` or its `{ … }` block, whichever
/// comes first outside parentheses and brackets (so `[u8; 4]` in a signature is skipped), or at the
/// `}` closing what holds it (a `#[cfg(test)]` field).
fn item_end(code: &[u8], start: usize) -> usize {
    let mut depth = 0_i32;
    let mut i = start;
    while i < code.len() {
        match code[i] {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b';' if depth == 0 => return i + 1,
            b'{' if depth == 0 => return block_end(code, i),
            b'}' if depth == 0 => return i,
            _ => {}
        }
        i += 1;
    }
    code.len()
}

fn block_end(code: &[u8], open: usize) -> usize {
    let mut depth = 0_i32;
    for (i, &byte) in code.iter().enumerate().skip(open) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
    }
    code.len()
}

fn blank(text: &str) -> String {
    text.chars().map(|c| if c == '\n' { '\n' } else { ' ' }).collect()
}

#[test]
fn comments_and_literals_are_blanked_with_lines_kept() {
    let code = code_only("a // crate::x\nb /* crate::y /* nested */ */ \"crate::z\" r#\"crate::w\"# '{' '\\'' f<'a>\nc");
    assert!(!code.contains("crate::"), "{code}");
    assert!(code.contains("f<'a>"), "a lifetime is code: {code}");
    assert_eq!(code.lines().count(), 3);
    assert_eq!(line_of(&code, code.find('c').unwrap()), 3);
}

#[test]
fn test_items_are_dropped() {
    let code = production(
        "use crate::a;\n#[cfg(test)]\nuse crate::b;\n#[cfg(test)]\nmod tests {\n    fn f() { crate::c(); }\n}\nfn g(x: [u8; 2]) {}\n",
    );
    assert!(code.contains("crate::a") && code.contains("fn g"), "{code}");
    assert!(!code.contains("crate::b") && !code.contains("crate::c"), "{code}");
    assert_eq!(production("#![cfg(test)]\nuse crate::a;").trim(), "");
}
