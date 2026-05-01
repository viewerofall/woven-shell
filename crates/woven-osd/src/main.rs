//! woven-osd — on-screen display for volume, brightness, and media.
//!
//! Usage:
//!   woven-osd          — start daemon (idempotent, exits if already running)
//!   woven-osd volume   — trigger volume OSD on running daemon
//!   woven-osd bright   — trigger brightness OSD
//!   woven-osd media    — trigger media OSD

mod daemon;
mod read;
mod render;
mod state;
mod surface;

use anyhow::Result;

pub const SOCK: &str = "/tmp/woven-osd.sock";

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let arg = std::env::args().nth(1);

    match arg.as_deref() {
        Some(cmd @ ("volume" | "bright" | "media")) => {
            // Client mode: forward command to running daemon
            client_send(cmd)
        }
        None => {
            // Daemon mode: start if not already running
            if daemon_alive() {
                tracing::info!("osd: daemon already running, exiting");
                return Ok(());
            }
            daemon::run()
        }
        Some(other) => {
            eprintln!("woven-osd: unknown command '{other}'");
            eprintln!("usage: woven-osd [volume|bright|media]");
            Ok(())
        }
    }
}

fn daemon_alive() -> bool {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    if let Ok(mut s) = UnixStream::connect(SOCK) {
        let _ = s.write_all(b"ping\n");
        true
    } else {
        false
    }
}

fn client_send(cmd: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    // Auto-start daemon if not running, then retry
    if !daemon_alive() {
        std::process::Command::new(std::env::current_exe()?)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        // Give it a moment to bind the socket
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    let mut s = UnixStream::connect(SOCK)
        .map_err(|_| anyhow::anyhow!("osd: daemon failed to start"))?;
    s.write_all(format!("{cmd}\n").as_bytes())?;
    Ok(())
}
