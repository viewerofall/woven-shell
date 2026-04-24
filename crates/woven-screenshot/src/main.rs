use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("full");

    // Create screenshots directory
    let pics_dir = PathBuf::from(std::env::var("HOME")?)
        .join("Pictures/Screenshots");
    fs::create_dir_all(&pics_dir)?;

    // Generate filename with timestamp
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();
    let filename = format!("screenshot_{}.png", now);
    let filepath = pics_dir.join(&filename);

    // Capture screenshot based on mode
    match mode {
        "area" => {
            // User selects area with slurp
            let slurp_out = Command::new("slurp").output()?;
            if !slurp_out.status.success() {
                anyhow::bail!("Screenshot cancelled");
            }
            let geometry = String::from_utf8_lossy(&slurp_out.stdout).trim().to_string();
            Command::new("grim")
                .args(&["-g", &geometry, filepath.to_string_lossy().as_ref()])
                .output()?;
        }
        "window" => {
            // Get focused window bounds
            let tree_out = Command::new("swaymsg")
                .args(&["-t", "get_tree"])
                .output()?;
            let tree: serde_json::Value = serde_json::from_slice(&tree_out.stdout)?;

            if let Some(rect) = find_focused_rect(&tree) {
                let geometry = format!("{},{} {}x{}", rect.0, rect.1, rect.2, rect.3);
                Command::new("grim")
                    .args(&["-g", &geometry, filepath.to_string_lossy().as_ref()])
                    .output()?;
            } else {
                anyhow::bail!("Could not find focused window");
            }
        }
        _ => {
            // Full screen
            Command::new("grim")
                .arg(filepath.to_string_lossy().as_ref())
                .output()?;
        }
    }

    println!("Screenshot saved: {}", filepath.display());

    // Copy to clipboard
    let _ = Command::new("wl-copy")
        .arg("--type")
        .arg("image/png")
        .arg(filepath.to_string_lossy().as_ref())
        .output();

    Ok(())
}

fn find_focused_rect(node: &serde_json::Value) -> Option<(i32, i32, i32, i32)> {
    if node["focused"].as_bool() == Some(true) {
        if let (Some(x), Some(y), Some(w), Some(h)) = (
            node["rect"]["x"].as_i64(),
            node["rect"]["y"].as_i64(),
            node["rect"]["width"].as_i64(),
            node["rect"]["height"].as_i64(),
        ) {
            return Some((x as i32, y as i32, w as i32, h as i32));
        }
    }

    if let Some(children) = node["nodes"].as_array() {
        for child in children {
            if let Some(rect) = find_focused_rect(child) {
                return Some(rect);
            }
        }
    }

    if let Some(children) = node["floating_nodes"].as_array() {
        for child in children {
            if let Some(rect) = find_focused_rect(child) {
                return Some(rect);
            }
        }
    }

    None
}
