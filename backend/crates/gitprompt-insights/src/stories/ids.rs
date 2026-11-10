//! Project ids: unpadded base64url (RFC 4648 §5) of the project's path, hand-rolled because these
//! two functions are all the crate needs of base64.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub(super) fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let group = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        // A chunk of n bytes carries n + 1 sextets; the rest would be padding, which is left out.
        for i in 0..=chunk.len() {
            let sextet = (group >> (18 - 6 * i)) & 0x3f;
            out.push(char::from(ALPHABET[sextet as usize]));
        }
    }
    out
}

fn sextet(c: u8) -> Option<u32> {
    ALPHABET.iter().position(|a| *a == c).and_then(|p| u32::try_from(p).ok())
}

/// The bytes of a base64url text (padding optional, at most two `=`); `None` for any other text.
pub(super) fn decode(key: &str) -> Option<Vec<u8>> {
    let body = key.trim_end_matches('=');
    let padded_len = key.len().div_ceil(4) * 4;
    if padded_len - body.len() > 2 || body.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    for chunk in body.as_bytes().chunks(4) {
        let group = chunk.iter().enumerate().try_fold(0u32, |acc, (i, c)| Some(acc | (sextet(*c)? << (18 - 6 * i))))?;
        // n sextets carry n - 1 whole bytes; leftover bits are ignored, as Python's decoder does.
        for i in 0..chunk.len() - 1 {
            // Masked to 8 bits: the cast cannot truncate anything that matters.
            out.push(((group >> (16 - 8 * i)) & 0xff) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding_reverses_encoding_with_or_without_padding() {
        for text in ["", "a", "ab", "abc", "abcd", "/tmp/a b/ü"] {
            let key = encode(text.as_bytes());
            assert_eq!(decode(&key).as_deref(), Some(text.as_bytes()));
        }
        assert_eq!(decode("YQ==").as_deref(), Some(&b"a"[..]));
        assert_eq!((decode("Y"), decode("YQ==="), decode("a+b/")), (None, None, None));
    }
}
