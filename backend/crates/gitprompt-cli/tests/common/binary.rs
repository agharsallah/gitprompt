//! Black-box helpers: the real `gitprompt` binary as a child process (exactly the environment set
//! here: `env_clear`), and HTTP/1.1 over a plain `TcpStream` to the daemon it runs. Nothing
//! in-process: what these helpers see is what a tool's hook or the dashboard sees.
//!
//! Every wait polls against a deadline (never a fixed sleep), and a daemon still running when its
//! [`Served`] is dropped is killed, so a failing test leaves no process behind.

use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::{Home, free_port};

/// How long a debug build may take to start answering `/healthz`.
pub const START_DEADLINE: Duration = Duration::from_secs(20);
/// How long one command (a hook, a refused `serve`) may run.
pub const COMMAND_DEADLINE: Duration = Duration::from_secs(10);
/// The pause between two polls of a condition.
const POLL: Duration = Duration::from_millis(20);
/// Socket timeout of one HTTP exchange.
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);

/// Polls `probe` until it returns `Some`, failing the test with `what` once `deadline` passes.
pub fn eventually<T>(deadline: Duration, what: &str, mut probe: impl FnMut() -> Option<T>) -> T {
    let until = Instant::now() + deadline;
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(Instant::now() < until, "timed out after {deadline:?} waiting for {what}");
        std::thread::sleep(POLL);
    }
}

impl Home {
    /// The binary with only this home's environment: HOME, `GITPROMPT_HOME`, `GITPROMPT_PORT`, and a
    /// PATH of the system folders (git for the git facts; no AI tool of the developer's machine).
    pub fn binary(&self, port: u16, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gitprompt"));
        command
            .env_clear()
            .env("HOME", &self.settings.user_home)
            .env("GITPROMPT_HOME", &self.settings.home)
            .env("GITPROMPT_PORT", port.to_string())
            .env("PATH", "/usr/bin:/bin")
            .args(args)
            .stdin(Stdio::null());
        command
    }

    /// Runs `gitprompt <args>` to completion with `stdin`; fails the test if it outlives
    /// [`COMMAND_DEADLINE`].
    pub fn run(&self, port: u16, args: &[&str], stdin: &[u8]) -> Output {
        let mut child =
            self.binary(port, args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("spawn gitprompt");
        let mut input = child.stdin.take().expect("stdin");
        input.write_all(stdin).expect("write stdin");
        drop(input); // EOF: the command reads stdin to the end
        let stdout = drain(child.stdout.take().expect("stdout"));
        let stderr = drain(child.stderr.take().expect("stderr"));
        let status = wait(&mut child, COMMAND_DEADLINE).unwrap_or_else(|| {
            let _ = child.kill();
            let _ = child.wait();
            panic!("gitprompt {args:?} still running after {COMMAND_DEADLINE:?}")
        });
        Output { status, stdout: stdout.join().expect("stdout reader"), stderr: stderr.join().expect("stderr reader") }
    }

    /// `gitprompt hook <source>` with `payload` on stdin, against a daemon (or nothing) on `port`.
    pub fn hook(&self, port: u16, source: &str, payload: &Value) -> Output {
        self.run(port, &["hook", source], payload.to_string().as_bytes())
    }

    /// `gitprompt serve` on a free loopback port, returned once `/healthz` answers 200.
    ///
    /// A port released by [`free_port`] can be taken before the daemon binds it (another test's
    /// connection may get it as its ephemeral port); only that failure is retried, on a new port.
    pub fn serve(&self) -> Served {
        const ATTEMPTS: usize = 5;
        for _ in 1..ATTEMPTS {
            let mut served = self.spawn_serve(free_port());
            match served.healthy() {
                Ok(()) => return served,
                Err(Exited { stderr, .. }) if stderr.contains("cannot listen on") => {}
                Err(exited) => panic!("{exited}"),
            }
        }
        let mut served = self.spawn_serve(free_port());
        served.wait_healthy();
        served
    }

    /// `gitprompt serve` on `port`, not waited for. Its stdout and stderr go to files next to the
    /// data directory (a pipe nobody reads could fill and block the daemon).
    pub fn spawn_serve(&self, port: u16) -> Served {
        static RUNS: AtomicUsize = AtomicUsize::new(0);
        let run = RUNS.fetch_add(1, Ordering::Relaxed);
        let root = self.settings.home.parent().expect("temp root").to_path_buf();
        let (out, err) = (root.join(format!("serve-{run}.stdout")), root.join(format!("serve-{run}.stderr")));
        let child = self
            .binary(port, &["serve"])
            .stdout(File::create(&out).expect("stdout file"))
            .stderr(File::create(&err).expect("stderr file"))
            .spawn()
            .expect("spawn gitprompt serve");
        Served { child: Some(child), port, out, err }
    }
}

fn drain(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        bytes
    })
}

/// The child's exit status, or `None` if it is still running after `deadline`.
fn wait(child: &mut Child, deadline: Duration) -> Option<ExitStatus> {
    let until = Instant::now() + deadline;
    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return Some(status);
        }
        if Instant::now() >= until {
            return None;
        }
        std::thread::sleep(POLL);
    }
}

/// A daemon that exited before answering `/healthz`.
pub struct Exited {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl std::fmt::Display for Exited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "gitprompt serve exited with {} before answering\nstdout: {}\nstderr: {}", self.status, self.stdout, self.stderr)
    }
}

/// A `gitprompt serve` child process.
pub struct Served {
    child: Option<Child>,
    pub port: u16,
    out: PathBuf,
    err: PathBuf,
}

impl Served {
    pub fn pid(&self) -> u32 {
        self.child.as_ref().expect("running").id()
    }

    /// What the daemon printed so far on stdout and stderr.
    pub fn output(&self) -> (String, String) {
        let read = |path: &Path| std::fs::read_to_string(path).unwrap_or_default();
        (read(&self.out), read(&self.err))
    }

    /// Polls `/healthz` until it answers 200; fails at once (with what it printed) if the daemon
    /// exits first.
    pub fn wait_healthy(&mut self) {
        if let Err(exited) = self.healthy() {
            panic!("{exited}");
        }
    }

    /// Polls `/healthz` until it answers 200 (`Ok`) or the daemon exits (`Err`, with its output);
    /// fails the test after [`START_DEADLINE`].
    fn healthy(&mut self) -> Result<(), Exited> {
        eventually(START_DEADLINE, "/healthz to answer 200", || {
            let child = self.child.as_mut().expect("running");
            if let Some(status) = child.try_wait().expect("try_wait") {
                self.child = None;
                let (stdout, stderr) = self.output();
                return Some(Err(Exited { status, stdout, stderr }));
            }
            request(self.port, "GET", "/healthz", &[], b"").ok().filter(|r| r.status == 200).map(|_| Ok(()))
        })
    }

    /// The exit status once the daemon has exited, within `deadline`.
    pub fn wait_exit(&mut self, deadline: Duration) -> Option<ExitStatus> {
        let child = self.child.as_mut().expect("running");
        let status = wait(child, deadline)?;
        self.child = None;
        Some(status)
    }

    /// Sends SIGINT, as Ctrl-C in a terminal does.
    #[cfg(unix)]
    pub fn interrupt(&self) {
        let status = Command::new("kill").args(["-INT", &self.pid().to_string()]).status().expect("run kill");
        assert!(status.success(), "kill -INT {} failed", self.pid());
    }

    /// Stops the daemon gracefully (SIGINT) where it can, else kills it; returns once it exited.
    pub fn stop(mut self) {
        #[cfg(unix)]
        self.interrupt();
        #[cfg(not(unix))]
        let _ = self.child.as_mut().expect("running").kill();
        let status = self.wait_exit(COMMAND_DEADLINE);
        assert!(status.is_some(), "gitprompt serve did not stop within {COMMAND_DEADLINE:?}");
    }

    pub fn get(&self, path: &str) -> Response {
        get(self.port, path)
    }

    pub fn json(&self, path: &str) -> Value {
        let response = self.get(path);
        assert_eq!(response.status, 200, "GET {path}: {}", response.text());
        response.json()
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// --- HTTP/1.1 over a TcpStream ---

pub struct Response {
    pub status: u16,
    /// Names lowercased.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|e| panic!("not JSON ({e}): {}", self.text()))
    }
}

/// The `Host` the daemon's host check accepts.
pub fn host(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

/// One request on a fresh connection (`Connection: close`); the whole response.
pub fn request(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: &[u8]) -> std::io::Result<Response> {
    let mut stream = connect(port)?;
    send_head(&mut stream, port, method, path, headers, body.len())?;
    stream.write_all(body)?;
    let mut reader = BufReader::new(stream);
    let (status, headers) = read_head(&mut reader)?;
    let header = |name: &str| headers.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str());
    let body = if header("transfer-encoding").is_some_and(|v| v.eq_ignore_ascii_case("chunked")) {
        read_chunked(&mut reader)?
    } else if let Some(length) = header("content-length") {
        let length: usize = length.parse().map_err(|_| invalid("bad content-length"))?;
        let mut body = vec![0; length];
        reader.read_exact(&mut body)?;
        body
    } else {
        let mut body = Vec::new();
        reader.read_to_end(&mut body)?;
        body
    };
    Ok(Response { status, headers, body })
}

/// A GET that must get an HTTP answer.
pub fn get(port: u16, path: &str) -> Response {
    request(port, "GET", path, &[], b"").unwrap_or_else(|e| panic!("GET {path}: {e}"))
}

pub fn connect(port: u16) -> std::io::Result<TcpStream> {
    let stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(HTTP_TIMEOUT))?;
    stream.set_write_timeout(Some(HTTP_TIMEOUT))?;
    Ok(stream)
}

pub fn send_head(
    stream: &mut TcpStream,
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    content_length: usize,
) -> std::io::Result<()> {
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n", host(port));
    if content_length > 0 || method != "GET" {
        write!(head, "Content-Length: {content_length}\r\n").unwrap();
    }
    for (name, value) in headers {
        write!(head, "{name}: {value}\r\n").unwrap();
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())
}

/// The status code and the headers (names lowercased) of a response.
pub fn read_head(reader: &mut impl BufRead) -> std::io::Result<(u16, Vec<(String, String)>)> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let status = line.split_whitespace().nth(1).and_then(|code| code.parse().ok()).ok_or_else(|| invalid("not an HTTP status line"))?;
    let mut headers = Vec::new();
    loop {
        line.clear();
        reader.read_line(&mut line)?;
        let header = line.trim_end();
        if header.is_empty() {
            return Ok((status, headers));
        }
        let (name, value) = header.split_once(':').ok_or_else(|| invalid("malformed header"))?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
    }
}

/// A `Transfer-Encoding: chunked` body, to its last chunk.
fn read_chunked(reader: &mut impl BufRead) -> std::io::Result<Vec<u8>> {
    let mut body = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        reader.read_line(&mut line)?;
        let size = line.trim().split(';').next().unwrap_or_default();
        let size = usize::from_str_radix(size, 16).map_err(|_| invalid("bad chunk size"))?;
        if size == 0 {
            return Ok(body);
        }
        let start = body.len();
        body.resize(start + size, 0);
        reader.read_exact(&mut body[start..])?;
        line.clear();
        reader.read_line(&mut line)?; // the CRLF after the chunk
    }
}

fn invalid(why: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, why.to_owned())
}

// --- connector fixtures (testdata/fixtures/<tool>/*.json) ---

/// A fixture: the tool, its file stem, and `{about, payload, expect}`.
pub struct Fixture {
    pub tool: String,
    pub name: String,
    pub case: Value,
}

/// Every fixture of every tool, tools and files in name order (the order they are replayed in).
pub fn fixtures() -> Vec<Fixture> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/fixtures");
    let mut out = Vec::new();
    for tool in sorted(&root) {
        let tool_name = tool.file_name().expect("tool dir").to_string_lossy().into_owned();
        for file in sorted(&tool).into_iter().filter(|p| p.extension().is_some_and(|e| e == "json")) {
            let name = file.file_stem().expect("stem").to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&file).expect("read fixture");
            let case = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            out.push(Fixture { tool: tool_name.clone(), name, case });
        }
    }
    out
}

fn sorted(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir).expect("read dir").map(|e| e.expect("entry").path()).collect();
    entries.sort();
    entries
}
