//! The forwarder's transport: one HTTP/1.1 POST over a plain `TcpStream` (the receiver is always
//! `http://127.0.0.1:<port>`), with the request's timeout for connecting, writing and reading the
//! status line. No HTTP client dependency for a single loopback request.

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};

use gitprompt_capture::forwarder::{HookRequest, Transport, Unreachable};

pub(crate) struct HttpTransport;

impl Transport for HttpTransport {
    fn send(&self, request: &HookRequest) -> Result<u16, Unreachable> {
        let unreachable = |e: std::io::Error| Unreachable(e.to_string());
        let rest = request.url.strip_prefix("http://").ok_or_else(|| Unreachable(format!("not an http URL: {}", request.url)))?;
        let (authority, path) = rest.split_once('/').map_or_else(|| (rest, "/".to_owned()), |(a, p)| (a, format!("/{p}")));
        let addr: SocketAddr =
            authority.to_socket_addrs().map_err(unreachable)?.next().ok_or_else(|| Unreachable(format!("no address for {authority}")))?;
        let mut stream = TcpStream::connect_timeout(&addr, request.timeout).map_err(unreachable)?;
        stream.set_read_timeout(Some(request.timeout)).map_err(unreachable)?;
        stream.set_write_timeout(Some(request.timeout)).map_err(unreachable)?;
        let mut head = format!(
            "{} {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nContent-Length: {}\r\n",
            request.method,
            request.body.len()
        );
        for (name, value) in &request.headers {
            let _ = write!(head, "{name}: {value}\r\n");
        }
        head.push_str("\r\n");
        stream.write_all(head.as_bytes()).map_err(unreachable)?;
        stream.write_all(&request.body).map_err(unreachable)?;
        let mut status_line = String::new();
        BufReader::new(stream).read_line(&mut status_line).map_err(unreachable)?;
        status_line
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .ok_or_else(|| Unreachable(format!("not an HTTP answer: {status_line:?}")))
    }
}

/// An HTTP answer read whole.
pub(crate) struct Answer {
    pub status: u16,
    pub body: Vec<u8>,
}

/// `GET <path>` on `addr` (loopback, `Host: 127.0.0.1:<port>`), the answer read until the server
/// closes the connection, everything within `timeout` per step. For `gitprompt doctor`.
pub(crate) fn get(addr: SocketAddr, path: &str, timeout: std::time::Duration) -> std::io::Result<Answer> {
    use std::io::{Error, ErrorKind, Read};
    let mut stream = TcpStream::connect_timeout(&addr, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    write!(stream, "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nAccept: application/json\r\n\r\n")?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let invalid = |what: &str| Error::new(ErrorKind::InvalidData, format!("not an HTTP answer: {what}"));
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or_else(|| invalid("no end of headers"))?;
    let head = String::from_utf8_lossy(raw.get(..split).unwrap_or_default()).into_owned();
    let body = raw.get(split + 4..).unwrap_or_default().to_vec();
    let status = head.split_whitespace().nth(1).and_then(|code| code.parse().ok()).ok_or_else(|| invalid("no status"))?;
    let chunked = head.lines().any(|line| {
        let line = line.to_ascii_lowercase();
        line.starts_with("transfer-encoding:") && line.contains("chunked")
    });
    let body = if chunked { dechunk(&body).ok_or_else(|| invalid("bad chunked body"))? } else { body };
    Ok(Answer { status, body })
}

/// A `Transfer-Encoding: chunked` body, decoded.
fn dechunk(mut rest: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = rest.windows(2).position(|w| w == b"\r\n")?;
        let size_text = std::str::from_utf8(rest.get(..line_end)?).ok()?;
        let size = usize::from_str_radix(size_text.split(';').next()?.trim(), 16).ok()?;
        rest = rest.get(line_end + 2..)?;
        if size == 0 {
            return Some(out);
        }
        out.extend_from_slice(rest.get(..size)?);
        rest = rest.get(size + 2..)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunked_bodies_are_joined() {
        assert_eq!(dechunk(b"4\r\nWiki\r\n5;x=1\r\npedia\r\n0\r\n\r\n"), Some(b"Wikipedia".to_vec()));
        assert_eq!(dechunk(b"0\r\n\r\n"), Some(Vec::new()));
        assert_eq!(dechunk(b"zz\r\n"), None);
        assert_eq!(dechunk(b"9\r\nshort\r\n"), None);
    }
}
