//! Is GitPrompt running for this data directory? The lock, the port, `/healthz`, `/api/status`.
//! Read-only: the lock is only tried (and released at once), nothing is created.

use std::fs::{File, TryLockError};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

use gitprompt_core::Settings;
use gitprompt_server::StatusOut;

use super::report::{DaemonCheck, LockCheck, RunningDaemon};
use crate::http;

/// How long each network step may take: the daemon is on loopback, so this only bounds a hang.
const TIMEOUT: Duration = Duration::from_millis(1500);

pub(super) fn check(settings: &Settings) -> DaemonCheck {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, settings.port));
    let listening = TcpStream::connect_timeout(&addr, TIMEOUT).is_ok();
    let healthz = listening && healthz(addr);
    let running = if healthz { running(addr) } else { None };
    let token_file = settings.token_file();
    DaemonCheck {
        port: settings.port,
        receiver: settings.receiver(),
        listening,
        healthz,
        running,
        lock: lock(&settings.lock_file()),
        token_present: std::fs::read_to_string(&token_file).is_ok_and(|t| !t.trim().is_empty()),
        token_file: token_file.display().to_string(),
    }
}

/// `/healthz` answers 200 `{"ok": true}`.
fn healthz(addr: SocketAddr) -> bool {
    http::get(addr, "/healthz", TIMEOUT).is_ok_and(|answer| {
        answer.status == 200
            && serde_json::from_slice::<serde_json::Value>(&answer.body).is_ok_and(|v| v.get("ok") == Some(&serde_json::Value::Bool(true)))
    })
}

/// The running daemon's `/api/status`; `None` when it has none (the legacy Python daemon).
fn running(addr: SocketAddr) -> Option<RunningDaemon> {
    let answer = http::get(addr, "/api/status", TIMEOUT).ok().filter(|a| a.status == 200)?;
    let status: StatusOut = serde_json::from_slice(&answer.body).ok()?;
    Some(RunningDaemon {
        version: status.version,
        backend: status.backend,
        started_at: status.started_at,
        uptime_s: status.uptime_s,
        home: status.home,
        dashboard: status.dashboard,
        health: status.health,
    })
}

/// Whether another process holds the lock, and the pid it wrote.
fn lock(path: &Path) -> LockCheck {
    let file_name = path.display().to_string();
    let Ok(file) = File::open(path) else { return LockCheck { file: file_name, held: false, pid: None, pid_alive: None } };
    // `try_lock` succeeds only when nobody holds the lock; the probe's own lock goes with `file`.
    let held = matches!(file.try_lock(), Err(TryLockError::WouldBlock));
    drop(file);
    let pid = std::fs::read_to_string(path).ok().and_then(|text| text.trim().parse::<u32>().ok());
    LockCheck { file: file_name, held, pid, pid_alive: pid.and_then(pid_alive) }
}

/// Whether `pid` is a running process; `None` when this platform can't tell.
fn pid_alive(pid: u32) -> Option<bool> {
    if pid == std::process::id() {
        return Some(true);
    }
    if Path::new("/proc/self").is_dir() {
        return Some(Path::new(&format!("/proc/{pid}")).exists());
    }
    if cfg!(windows) {
        let output = std::process::Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH"]).output().ok()?;
        return Some(String::from_utf8_lossy(&output.stdout).split_whitespace().any(|word| word == pid.to_string()));
    }
    // macOS and the BSDs: `ps -p` exits 0 when the process exists (whoever owns it).
    let ps = ["/bin/ps", "/usr/bin/ps"].into_iter().find(|p| Path::new(p).is_file()).unwrap_or("ps");
    let status = std::process::Command::new(ps)
        .args(["-p", &pid.to_string(), "-o", "pid="])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    Some(status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_is_alive_and_an_exited_child_is_not() {
        assert_eq!(pid_alive(std::process::id()), Some(true));
        let mut child =
            std::process::Command::new(std::env::current_exe().unwrap()).arg("--list").stdout(std::process::Stdio::null()).spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        assert_eq!(pid_alive(pid), Some(false));
    }

    #[test]
    fn a_missing_lock_file_is_not_held() {
        let tmp = tempfile::tempdir().unwrap();
        let check = lock(&tmp.path().join("daemon.lock"));
        assert!(!check.held);
        assert_eq!(check.pid, None);
    }

    #[test]
    fn an_unheld_lock_file_reports_its_stale_pid() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("daemon.lock");
        std::fs::write(&path, "4242").unwrap();
        let check = lock(&path);
        assert!(!check.held);
        assert_eq!(check.pid, Some(4242));
        // The probe released its own lock.
        assert!(File::open(&path).unwrap().try_lock().is_ok());
    }
}
