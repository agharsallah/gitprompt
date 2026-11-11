//! Randomness from the operating system's CSPRNG, for secrets and unique file names.

use std::fmt::Write as _;
use std::io;

/// `N` random bytes from the OS CSPRNG.
pub(crate) fn bytes<const N: usize>() -> io::Result<[u8; N]> {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).map_err(|e| io::Error::other(format!("no randomness from the operating system: {e}")))?;
    Ok(buf)
}

/// A URL-safe text secret from `N` random bytes (unpadded base64url, like Python's
/// `secrets.token_urlsafe`).
pub(crate) fn token_urlsafe<const N: usize>() -> io::Result<String> {
    Ok(base64url(&bytes::<N>()?))
}

/// A short random hex suffix for temporary file names.
pub(crate) fn hex_suffix() -> io::Result<String> {
    Ok(bytes::<8>()?.iter().fold(String::with_capacity(16), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    }))
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Unpadded base64url (RFC 4648 §5).
fn base64url(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, &b)| acc | (u32::from(b) << (16 - 8 * i)));
        // A chunk of k bytes carries k + 1 base64 digits.
        for i in 0..=chunk.len() {
            #[expect(clippy::indexing_slicing, reason = "masked to 6 bits, so always below the alphabet's 64")]
            out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 0x3f) as usize]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_matches_rfc_4648_vectors_without_padding() {
        let cases =
            [("", ""), ("f", "Zg"), ("fo", "Zm8"), ("foo", "Zm9v"), ("foob", "Zm9vYg"), ("fooba", "Zm9vYmE"), ("foobar", "Zm9vYmFy")];
        for (input, expected) in cases {
            assert_eq!(base64url(input.as_bytes()), expected);
        }
    }

    #[test]
    fn base64url_uses_the_url_safe_alphabet() {
        assert_eq!(base64url(&[0xfb, 0xff, 0xbf]), "-_-_");
    }

    #[test]
    fn a_32_byte_token_has_43_characters_and_differs_each_time() {
        let (a, b) = (token_urlsafe::<32>().unwrap(), token_urlsafe::<32>().unwrap());
        assert_eq!((a.len(), b.len()), (43, 43));
        assert_ne!(a, b);
    }
}
