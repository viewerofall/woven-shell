//! woven-cfg — config manager for woven-shell.
//! Toggle: second invocation kills the running instance.

mod config;
mod panel;
mod surface;
mod tabs;
mod widgets;

use anyhow::Result;

const LOCK: &str = "/tmp/woven-cfg.pid";

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    if let Ok(s) = std::fs::read_to_string(LOCK) {
        if let Ok(pid) = s.trim().parse::<i32>() {
            if unsafe { libc::kill(pid, 0) } == 0 {
                unsafe { libc::kill(pid, libc::SIGTERM); }
                let _ = std::fs::remove_file(LOCK);
                return Ok(());
            }
        }
        let _ = std::fs::remove_file(LOCK);
    }

    let _ = std::fs::write(LOCK, std::process::id().to_string());
    let _guard = PidGuard;

    let mut surf = surface::CfgSurface::new()?;
    loop {
        if surf.tick()? { break; }
    }
    Ok(())
}

struct PidGuard;
impl Drop for PidGuard {
    fn drop(&mut self) { let _ = std::fs::remove_file(LOCK); }
}
