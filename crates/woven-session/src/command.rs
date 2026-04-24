use anyhow::{bail, Result};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::Duration;

pub async fn run_command(args: &[String]) -> Result<()> {
    if args.is_empty() {
        bail!("no command provided");
    }

    match args[0].as_str() {
        "status" => cmd_status(),
        "reload" => cmd_reload(),
        "quit" => cmd_quit(),

        // Media
        "media" => {
            if args.len() < 2 {
                bail!("usage: woven-session media <play|pause|toggle|next|prev|status>");
            }
            cmd_media(&args[1])
        }

        // Volume
        "volume" => {
            if args.len() < 2 {
                bail!("usage: woven-session volume <get|set|up|down|mute|unmute>");
            }
            cmd_volume(&args[1..])
        }

        // Brightness
        "brightness" => {
            if args.len() < 2 {
                bail!("usage: woven-session brightness <get|set|up|down>");
            }
            cmd_brightness(&args[1..])
        }

        // Power
        "power" => {
            if args.len() < 2 {
                bail!("usage: woven-session power <suspend|poweroff|reboot|lock>");
            }
            cmd_power(&args[1])
        }

        // Config
        "config" => {
            if args.len() < 2 {
                bail!("usage: woven-session config <reload|list>");
            }
            cmd_config(&args[1])
        }

        // OSD
        "osd" => {
            if args.len() < 2 {
                bail!("usage: woven-session osd <volume|brightness|media>");
            }
            cmd_osd(&args[1])
        }

        // Meta
        "--help" | "-h" | "help" => print_help(),
        "--version" | "-v" | "version" => print_version(),
        "--update" => cmd_update().await,

        other => bail!("unknown command: {}", other),
    }
}

fn query_session(cmd: &str) -> Result<String> {
    let mut stream = UnixStream::connect("/tmp/woven-session.sock")?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;

    stream.write_all(format!("{}\n", cmd).as_bytes())?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line.trim().to_string())
}

fn cmd_status() -> Result<()> {
    let resp = query_session("get_all")?;
    println!("{}", resp);
    Ok(())
}

fn cmd_reload() -> Result<()> {
    println!("Reloading session...");
    // Signal daemon to reload via socket command
    query_session("reload")?;
    println!("Session reloaded");
    Ok(())
}

fn cmd_quit() -> Result<()> {
    println!("Shutting down session...");
    query_session("quit")?;
    Ok(())
}

fn cmd_media(subcmd: &str) -> Result<()> {
    match subcmd {
        "play" => run("playerctl", &["play"])?,
        "pause" => run("playerctl", &["pause"])?,
        "toggle" => run("playerctl", &["play-pause"])?,
        "next" => run("playerctl", &["next"])?,
        "prev" => run("playerctl", &["previous"])?,
        "status" => {
            let resp = query_session("get_media")?;
            println!("{}", resp);
        }
        _ => bail!("unknown media subcommand: {}", subcmd),
    }
    Ok(())
}

fn cmd_volume(args: &[String]) -> Result<()> {
    match args[0].as_str() {
        "get" => {
            let resp = query_session("get_volume")?;
            println!("{}", resp);
        }
        "set" => {
            if args.len() < 2 {
                bail!("usage: woven-session volume set <percent>");
            }
            run("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{}%", args[1])])?;
        }
        "up" => {
            let amount = args.get(1).map(|s| s.as_str()).unwrap_or("5");
            run("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("+{}%", amount)])?;
        }
        "down" => {
            let amount = args.get(1).map(|s| s.as_str()).unwrap_or("5");
            run("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("-{}%", amount)])?;
        }
        "mute" => run("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "1"])?,
        "unmute" => run("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "0"])?,
        _ => bail!("unknown volume subcommand: {}", args[0]),
    }
    Ok(())
}

fn cmd_brightness(args: &[String]) -> Result<()> {
    match args[0].as_str() {
        "get" => {
            let resp = query_session("get_battery")?; // Battery state includes brightness in context
            println!("{}", resp);
        }
        "set" => {
            if args.len() < 2 {
                bail!("usage: woven-session brightness set <percent>");
            }
            run("brightnessctl", &["set", &format!("{}%", args[1])])?;
        }
        "up" => {
            let amount = args.get(1).map(|s| s.as_str()).unwrap_or("10");
            run("brightnessctl", &["set", &format!("+{}%", amount)])?;
        }
        "down" => {
            let amount = args.get(1).map(|s| s.as_str()).unwrap_or("10");
            run("brightnessctl", &["set", &format!("-{}%", amount)])?;
        }
        _ => bail!("unknown brightness subcommand: {}", args[0]),
    }
    Ok(())
}

fn cmd_power(subcmd: &str) -> Result<()> {
    match subcmd {
        "suspend" => run("systemctl", &["suspend"])?,
        "poweroff" => run("systemctl", &["poweroff"])?,
        "reboot" => run("systemctl", &["reboot"])?,
        "lock" => run_bg("~/.local/bin/woven-lock")?,
        _ => bail!("unknown power subcommand: {}", subcmd),
    }
    Ok(())
}

fn cmd_config(subcmd: &str) -> Result<()> {
    match subcmd {
        "reload" => {
            // Reload sway config gracefully
            run("swaymsg", &["reload"])?;
            println!("Config reloaded and sway restarted");
        }
        "list" => {
            println!("Available configs:");
            println!("  ~/.config/woven-shell/keybinds.toml");
            println!("  ~/.config/woven-shell/bar.toml");
            println!("  ~/.config/woven-shell/wall.toml");
            println!("  ~/.config/woven-shell/lock.toml");
            println!("  ~/.config/woven-shell/launch.toml");
        }
        _ => bail!("unknown config subcommand: {}", subcmd),
    }
    Ok(())
}

fn cmd_osd(subcmd: &str) -> Result<()> {
    // OSD is woven-osd daemon — just notify it
    match subcmd {
        "volume" | "brightness" | "media" => {
            run_bg(&format!("~/.local/bin/woven-osd {}", subcmd))?;
        }
        _ => bail!("unknown osd subcommand: {}", subcmd),
    }
    Ok(())
}

async fn cmd_update() -> Result<()> {
    println!("Checking for updates...");
    let output = tokio::process::Command::new("curl")
        .args(&[
            "-fsSL",
            "https://api.github.com/repos/viewerofall/woven-shell/releases/latest",
        ])
        .output()
        .await?;

    if !output.status.success() {
        bail!("failed to check for updates");
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let latest_tag = json["tag_name"].as_str().ok_or(anyhow::anyhow!("no tag found"))?;
    let current_version = env!("CARGO_PKG_VERSION");

    if latest_tag == current_version {
        println!("Already up to date: {}", current_version);
        return Ok(());
    }

    println!("Updating {} → {}", current_version, latest_tag);

    // Download and extract tarball
    let tmp_dir = std::env::temp_dir().join("woven-shell-update");
    std::fs::create_dir_all(&tmp_dir)?;

    let tar_url = format!(
        "https://github.com/viewerofall/woven-shell/releases/latest/download/woven-shell.tar.gz"
    );
    let tar_path = tmp_dir.join("woven-shell.tar.gz");

    tokio::process::Command::new("curl")
        .args(&["-fsSL", &tar_url, "-o", tar_path.to_string_lossy().as_ref()])
        .output()
        .await?;

    tokio::process::Command::new("tar")
        .args(&["-xzf", tar_path.to_string_lossy().as_ref(), "-C", tmp_dir.to_string_lossy().as_ref()])
        .output()
        .await?;

    // Find extracted dir
    let extracted = std::fs::read_dir(&tmp_dir)?
        .find(|e| {
            if let Ok(e) = e {
                e.file_type().ok().map(|ft| ft.is_dir()).unwrap_or(false)
                    && e.file_name().to_string_lossy().contains("woven")
            } else {
                false
            }
        })
        .ok_or(anyhow::anyhow!("no extracted directory found"))?;

    let src_bin = extracted?.path().join("bin/woven-session");
    let dst_bin = std::path::PathBuf::from(std::env::var("HOME")?)
        .join(".local/bin/woven-session");

    std::fs::copy(&src_bin, &dst_bin)?;
    std::fs::set_permissions(&dst_bin, std::fs::Permissions::from_mode(0o755))?;

    println!("✓ Updated to {}", latest_tag);
    println!("Restart woven-session to apply changes: systemctl --user restart woven-session");

    // Cleanup
    let _ = std::fs::remove_dir_all(&tmp_dir);
    Ok(())
}

fn run(cmd: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(cmd).args(args).output()?;
    if !output.status.success() {
        bail!("command failed: {} {:?}", cmd, args);
    }
    Ok(())
}

fn run_bg(cmd: &str) -> Result<()> {
    Command::new("sh").arg("-c").arg(cmd).spawn()?;
    Ok(())
}

fn print_help() -> Result<()> {
    println!(
        r#"woven-session — unified Wayland shell daemon + command manager

USAGE
  woven-session [COMMAND] [ARGS...]
  woven-session                    # Run as daemon
  woven-session media play         # Execute command

COMMANDS
  Media:
    media play                     # Play
    media pause                    # Pause
    media toggle                   # Play/pause
    media next                     # Next track
    media prev                     # Previous track
    media status                   # Show current track

  Volume:
    volume get                     # Current volume %
    volume set <N>                 # Set to N%
    volume up [N]                  # Increase by N% (default 5)
    volume down [N]                # Decrease by N% (default 5)
    volume mute                    # Mute
    volume unmute                  # Unmute

  Brightness:
    brightness get                 # Current brightness %
    brightness set <N>             # Set to N%
    brightness up [N]              # Increase by N% (default 10)
    brightness down [N]            # Decrease by N% (default 10)

  Power:
    power suspend                  # Suspend
    power poweroff                 # Shut down
    power reboot                   # Reboot
    power lock                     # Lock screen

  Config:
    config reload                  # Hot-reload configs + restart sway
    config list                    # Show available configs

  OSD:
    osd volume                     # Show volume OSD
    osd brightness                 # Show brightness OSD
    osd media                      # Show media OSD

  Session:
    status                         # Show all current state
    reload                         # Reload session
    quit                           # Graceful shutdown

  Meta:
    --help, -h, help               # Show this message
    --version, -v, version         # Show version
    --update                       # Update from GitHub

EXAMPLES
  Keybind to increase volume:
    bindsym XF86AudioRaiseVolume exec woven-session volume up 5

  Change brightness:
    woven-session brightness set 75

  Next track:
    woven-session media next
"#
    );
    Ok(())
}

fn print_version() -> Result<()> {
    println!("woven-session v{}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
