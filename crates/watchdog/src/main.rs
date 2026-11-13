//! `rivet-watchdog`: keeps Rivet running on the printer.
//!
//! ```text
//! rivet-watchdog --dir DIR --log CONSOLE_LOG --child-pid FILE -- RIVET_ARGS...
//! ```
//!
//! Started by `S80rivet` (through `start-stop-daemon`), it runs `DIR/rivet` and
//! blocks until it exits: no polling, no helper processes. Then:
//!
//! - before every start, a `rivet` that does not match `rivet.sha256` is
//!   replaced by `rivet.good` (a broken download is never run);
//! - a `rivet` that has run [`HEALTHY`] is kept as `rivet.good`;
//! - a new `rivet` that exits with an error twice within [`HEALTHY`] is replaced
//!   by `rivet.good`, and `rolled-back` records it for the screen;
//! - restarts wait 1 s, doubling after each quick crash up to [`MAX_DELAY`]; an
//!   exit with code 0 is a restart on purpose (an update) and starts at once.
//!
//! SIGTERM / SIGINT (`S80rivet stop`, or `killall rivet-watchdog`) stop Rivet,
//! wait for it, point the framebuffer back at its first page (a screen started
//! next may keep the offset it finds) and exit.
//!
//! Rivet's own output and these notes go to CONSOLE_LOG, cut to one old copy
//! past 256 KB.

mod sha256;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicI32, AtomicPtr, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// A run this long counts as healthy.
const HEALTHY: Duration = Duration::from_secs(30);
/// The longest wait between restarts.
const MAX_DELAY: Duration = Duration::from_secs(60);
/// The console log is cut past this size.
const LOG_LIMIT: u64 = 256 * 1024;

/// Rivet's pid while it runs, for the signal handler.
static CHILD: AtomicI32 = AtomicI32::new(0);
/// The child pid file, as a leaked C string, for the signal handler.
static CHILD_PID_FILE: AtomicPtr<libc::c_char> = AtomicPtr::new(std::ptr::null_mut());

struct Paths {
    rivet: PathBuf,
    good: PathBuf,
    sum: PathBuf,
    rolled: PathBuf,
    log: PathBuf,
    child_pid: PathBuf,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let split = args.iter().position(|a| a == "--").unwrap_or(args.len());
    let (opts, rivet_args) = (&args[..split], args.get(split + 1..).unwrap_or(&[]));
    let opt = |name: &str| {
        opts.iter()
            .position(|a| a == name)
            .and_then(|i| opts.get(i + 1))
            .cloned()
    };
    let Some(dir) = opt("--dir").map(PathBuf::from) else {
        eprintln!("usage: rivet-watchdog --dir DIR --log FILE --child-pid FILE -- RIVET_ARGS...");
        std::process::exit(2);
    };
    let p = Paths {
        rivet: dir.join("rivet"),
        good: dir.join("rivet.good"),
        sum: dir.join("rivet.sha256"),
        rolled: dir.join("rolled-back"),
        log: opt("--log").map_or_else(|| dir.join("rivet-console.log"), PathBuf::from),
        child_pid: opt("--child-pid")
            .map_or_else(|| PathBuf::from("/run/rivet.child.pid"), PathBuf::from),
    };
    install_signal_handlers(&p.child_pid);
    supervise(&p, rivet_args);
}

fn supervise(p: &Paths, rivet_args: &[String]) -> ! {
    let mut delay = Duration::from_secs(1);
    let mut fails = 0u32;
    // The binary as last checked (size, modified): checking 14 MB on the
    // printer's CPU takes seconds, so only a changed file is checked again.
    let mut checked: Option<(u64, std::time::SystemTime)> = None;
    loop {
        cut_log(&p.log);
        let stamp = fs::metadata(&p.rivet)
            .ok()
            .and_then(|m| Some((m.len(), m.modified().ok()?)));
        if stamp.is_none() || stamp != checked {
            check_binary(p);
            checked = fs::metadata(&p.rivet)
                .ok()
                .and_then(|m| Some((m.len(), m.modified().ok()?)));
        }
        let started = Instant::now();
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p.log)
            .ok();
        let (out, err) = match &log {
            Some(f) => (
                f.try_clone().map_or_else(|_| Stdio::null(), Stdio::from),
                f.try_clone().map_or_else(|_| Stdio::null(), Stdio::from),
            ),
            None => (Stdio::null(), Stdio::null()),
        };
        let child = Command::new(&p.rivet)
            .args(rivet_args)
            .env("RIVET_SUPERVISED", "1")
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err)
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                say(p, &format!("cannot start {}: {e}", p.rivet.display()));
                fails += 1;
                if fails >= 2 {
                    roll_back(p, "rivet does not start");
                    fails = 0;
                }
                std::thread::sleep(delay);
                delay = (delay * 2).min(MAX_DELAY);
                continue;
            }
        };
        let pid = child.id() as i32;
        CHILD.store(pid, Ordering::SeqCst);
        let _ = fs::write(&p.child_pid, format!("{pid}\n"));
        say(p, &format!("started rivet (pid {pid})"));

        // Wait for the exit on a thread; here, wait for it or for HEALTHY.
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(child.wait());
        });
        let status = match rx.recv_timeout(HEALTHY) {
            Ok(s) => s,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                fails = 0;
                if !same_file(&p.rivet, &p.good) && copy_atomic(&p.rivet, &p.good).is_ok() {
                    let _ = fs::remove_file(&p.rolled);
                    say(
                        p,
                        &format!("rivet ran {}s: kept as rivet.good", HEALTHY.as_secs()),
                    );
                }
                rx.recv()
                    .unwrap_or_else(|_| Err(std::io::ErrorKind::Other.into()))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(std::io::ErrorKind::Other.into()),
        };
        CHILD.store(0, Ordering::SeqCst);
        let _ = fs::remove_file(&p.child_pid);
        let ran = started.elapsed();
        let code = status.ok().and_then(|s| s.code());
        if ran >= HEALTHY || code == Some(0) {
            // Ran long enough, or exited on purpose (a new binary to start).
            delay = Duration::from_secs(1);
        } else if p.good.exists() && !same_file(&p.rivet, &p.good) {
            // A new binary that does not come up: two tries, then the old one.
            fails += 1;
            if fails >= 2 {
                roll_back(
                    p,
                    &format!("new rivet exited twice within {}s", HEALTHY.as_secs()),
                );
                fails = 0;
            }
        }
        let code_text = code.map_or_else(|| "a signal".to_owned(), |c| format!("code {c}"));
        say(
            p,
            &format!(
                "rivet exited with {code_text} after {}s, restarting in {}s",
                ran.as_secs(),
                delay.as_secs()
            ),
        );
        std::thread::sleep(delay);
        if ran < HEALTHY && code != Some(0) {
            delay = (delay * 2).min(MAX_DELAY);
        }
    }
}

/// Before a start: a binary that does not match its checksum is not run.
fn check_binary(p: &Paths) {
    if p.good.exists() && same_file(&p.rivet, &p.good) {
        return; // the one that worked (after a roll-back the checksum is the bad one's)
    }
    let Ok(text) = fs::read_to_string(&p.sum) else {
        return;
    };
    let Some(want) = text.split_whitespace().next() else {
        return;
    };
    let have = fs::read(&p.rivet)
        .map(|b| sha256::hex(&b))
        .unwrap_or_default();
    if !have.eq_ignore_ascii_case(want) {
        roll_back(p, "rivet does not match rivet.sha256 (broken download?)");
    }
}

/// Back to the last binary that worked, if there is one; say why.
fn roll_back(p: &Paths, why: &str) {
    if p.good.exists() && !same_file(&p.good, &p.rivet) {
        let bad = fs::read(&p.rivet)
            .map(|b| sha256::hex(&b))
            .unwrap_or_default();
        let _ = fs::write(&p.rolled, format!("{bad}\n"));
        match copy_atomic(&p.good, &p.rivet) {
            Ok(()) => say(
                p,
                &format!("{why}; rolled back to the last binary that worked"),
            ),
            Err(e) => say(p, &format!("{why}; roll-back failed: {e}")),
        }
    } else {
        say(p, &format!("{why}; no other binary to roll back to"));
    }
}

/// Copy next to the target, then rename over it; keep it executable.
fn copy_atomic(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let tmp = to.with_extension("tmp");
    fs::copy(from, &tmp)?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    fs::rename(&tmp, to)
}

fn same_file(a: &Path, b: &Path) -> bool {
    // Different sizes: different files, without reading 14 MB twice.
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(x), Ok(y)) if x.len() != y.len() => return false,
        (Err(_), _) | (_, Err(_)) => return false,
        _ => {}
    }
    match (fs::read(a), fs::read(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

fn cut_log(log: &Path) {
    if fs::metadata(log).is_ok_and(|m| m.len() > LOG_LIMIT) {
        let _ = fs::rename(log, log.with_extension("log.1"));
    }
}

// libc::time_t is deprecated in name only (it widens in a later musl).
#[allow(deprecated)]
fn say(p: &Paths, msg: &str) {
    let mut t: libc::time_t = 0;
    // SAFETY: time() and localtime_r() write only into the locals we pass.
    let stamp = unsafe {
        libc::time(&mut t);
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min,
            tm.tm_sec
        )
    };
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&p.log) {
        let _ = writeln!(f, "{stamp} watchdog: {msg}");
    }
}

fn install_signal_handlers(child_pid: &Path) {
    if let Ok(c) = std::ffi::CString::new(child_pid.to_string_lossy().into_owned()) {
        CHILD_PID_FILE.store(c.into_raw(), Ordering::SeqCst);
    }
    let handler = on_stop as extern "C" fn(libc::c_int);
    // SAFETY: installing a handler that only makes async-signal-safe calls.
    unsafe {
        libc::signal(libc::SIGTERM, handler as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, handler as *const () as libc::sighandler_t);
    }
}

/// SIGTERM / SIGINT: only async-signal-safe calls.
extern "C" fn on_stop(_: libc::c_int) {
    // SAFETY: kill, waitpid, open, write, close, unlink and _exit are
    // async-signal-safe; the strings are static and NUL-terminated.
    unsafe {
        let pid = CHILD.load(Ordering::SeqCst);
        if pid > 0 {
            libc::kill(pid, libc::SIGTERM);
            let mut status = 0;
            libc::waitpid(pid, &mut status, 0);
        }
        let pan = c"/sys/class/graphics/fb0/pan";
        let fd = libc::open(pan.as_ptr(), libc::O_WRONLY);
        if fd >= 0 {
            libc::write(fd, c"0,0".as_ptr().cast(), 3);
            libc::close(fd);
        }
        let file = CHILD_PID_FILE.load(Ordering::SeqCst);
        if !file.is_null() {
            libc::unlink(file);
        }
        libc::_exit(0);
    }
}
