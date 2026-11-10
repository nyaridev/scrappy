//! Launch: reaching the app's socket, and starting the app first when nothing is listening. The app sits beside this
//! program (the same folder in a build, an install or a macOS bundle), or an `app-path` file beside it names the app
//! (an AppImage's copy of this host, ADR-0014).

// ── Imports ───────────────────────────────
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use interprocess::local_socket::{GenericNamespaced, Stream, prelude::*};

// ── Constants ─────────────────────────────
#[cfg(windows)]
const APP_NAMES: &[&str] = &["scrappy.exe", "Scrappy.exe"];
#[cfg(not(windows))]
const APP_NAMES: &[&str] = &["scrappy", "Scrappy"];
const START_WAIT: Duration = Duration::from_secs(20);
const RETRY: Duration = Duration::from_millis(250);

// ── Private helpers ───────────────────────
/// The app's socket name; `scrappy-core::bridge::socket_name` builds the same one.
fn socket_name() -> String {
    let user = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default();
    let user: String = user.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    format!("scrappy-{}.sock", if user.is_empty() { "user" } else { &user })
}

fn connect() -> io::Result<Stream> {
    Stream::connect(socket_name().to_ns_name::<GenericNamespaced>()?)
}

fn app_path() -> io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().ok_or(io::ErrorKind::NotFound)?;
    let named = || std::fs::read_to_string(dir.join("app-path")).ok().map(|path| PathBuf::from(path.trim()));
    APP_NAMES
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
        .or_else(|| named().filter(|path| path.is_file()))
        .ok_or_else(|| io::ErrorKind::NotFound.into())
}

/// Starts the app on its own, so it outlives this host when the browser closes the connection.
fn start_app() -> io::Result<()> {
    let mut command = Command::new(app_path()?);
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        // Chrome runs hosts in a job that ends with them; leave it when the job allows that.
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB);
        if command.spawn().is_ok() {
            return Ok(());
        }
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    command.spawn().map(drop)
}

// ── Public API ────────────────────────────
/// The app's socket, starting the app and waiting for it when nothing listens yet.
pub fn connect_or_launch() -> io::Result<Stream> {
    if let Ok(stream) = connect() {
        return Ok(stream);
    }
    start_app()?;
    let started = Instant::now();
    loop {
        thread::sleep(RETRY);
        match connect() {
            Ok(stream) => return Ok(stream),
            Err(error) if started.elapsed() > START_WAIT => return Err(error),
            Err(_) => {}
        }
    }
}
