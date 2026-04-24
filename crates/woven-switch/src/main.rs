use anyhow::Result;
use std::process::Command;

fn main() -> Result<()> {
    // Get workspaces from sway
    let output = Command::new("swaymsg")
        .args(&["-t", "get_workspaces"])
        .output()?;

    if !output.status.success() {
        anyhow::bail!("swaymsg failed");
    }

    let workspaces: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let mut ws_list: Vec<(u32, String, bool)> = Vec::new();

    if let Some(arr) = workspaces.as_array() {
        for ws in arr {
            if let (Some(num), Some(name), Some(focused)) = (
                ws["num"].as_u64(),
                ws["name"].as_str(),
                ws["focused"].as_bool(),
            ) {
                ws_list.push((num as u32, name.to_string(), focused));
            }
        }
    }

    // Sort by workspace number
    ws_list.sort_by_key(|w| w.0);

    if ws_list.is_empty() {
        return Ok(());
    }

    // Find current workspace
    let current = ws_list.iter().position(|w| w.2).unwrap_or(0);

    // Simple cycle: go to next workspace, wrap around
    let next = (current + 1) % ws_list.len();
    let target_ws = &ws_list[next].1;

    Command::new("swaymsg")
        .arg(format!("workspace {}", target_ws))
        .spawn()?;

    Ok(())
}
