//! woven-cc — control center popup.
//! Toggle: if already running, kills the existing instance and exits.
//! Appears anchored top-right, below the bar.

mod panel;
mod surface;

use anyhow::Result;

const LOCK: &str = "/tmp/woven-cc.pid";
const BAR_HEIGHT: u32 = 38; // bar height + small gap

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Toggle: kill existing instance if running
    if let Ok(s) = std::fs::read_to_string(LOCK) {
        if let Ok(pid) = s.trim().parse::<i32>() {
            let running = unsafe { libc::kill(pid, 0) } == 0;
            if running {
                unsafe { libc::kill(pid, libc::SIGTERM); }
                let _ = std::fs::remove_file(LOCK);
                return Ok(());
            }
        }
        let _ = std::fs::remove_file(LOCK);
    }

    // Write PID lock
    let _ = std::fs::write(LOCK, std::process::id().to_string());

    // Clean up lock on exit
    let _guard = PidGuard;

    let mut surf = surface::CcSurface::new(BAR_HEIGHT)?;

    loop {
        if surf.tick()? { break; }
    }

    Ok(())
}

struct PidGuard;
impl Drop for PidGuard {
    fn drop(&mut self) { let _ = std::fs::remove_file(LOCK); }
}
