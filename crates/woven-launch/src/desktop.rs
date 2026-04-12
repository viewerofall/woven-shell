//! Parse .desktop files from standard XDG application directories.

use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DesktopEntry {
    pub name: String,
    pub exec: String,
    pub comment: String,
    pub icon: String,
    pub categories: Vec<String>,
    pub terminal: bool,
    pub path: PathBuf,
}

impl DesktopEntry {
    /// Short category label for display (first category or inferred).
    pub fn category_label(&self) -> &str {
        for cat in &self.categories {
            match cat.as_str() {
                "WebBrowser" | "Network" => return "Browser",
                "TerminalEmulator" | "System" => return "System",
                "TextEditor" | "Development" | "IDE" => return "Editor",
                "FileManager" | "Utility" => return "Utility",
                "Graphics" => return "Graphics",
                "AudioVideo" | "Audio" | "Video" | "Player" => return "Media",
                "Game" => return "Game",
                "Settings" => return "Settings",
                "Office" => return "Office",
                _ => {}
            }
        }
        "App"
    }
}

/// Collect all .desktop entries from XDG application directories.
pub fn collect_entries() -> Vec<DesktopEntry> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut entries = Vec::new();

    for dir in app_dirs() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") { continue; }
            let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            if let Some(de) = parse_desktop_file(&path) {
                if let Some(&idx) = seen.get(&fname) {
                    // later dirs override earlier (user > system)
                    entries[idx] = de;
                } else {
                    seen.insert(fname, entries.len());
                    entries.push(de);
                }
            }
        }
    }

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries
}

fn app_dirs() -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let xdg_data = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".into());

    let mut dirs = vec![format!("{home}/.local/share/applications")];
    for d in xdg_data.split(':') {
        dirs.push(format!("{d}/applications"));
    }
    dirs
}

fn parse_desktop_file(path: &PathBuf) -> Option<DesktopEntry> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let mut name = String::new();
    let mut exec = String::new();
    let mut comment = String::new();
    let mut icon = String::new();
    let mut categories = String::new();
    let mut terminal = false;
    let mut no_display = false;
    let mut hidden = false;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            if in_entry { break; } // only parse [Desktop Entry]
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry { continue; }

        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim();
            let val = val.trim();
            match key {
                "Name" => name = val.to_string(),
                "Exec" => exec = val.to_string(),
                "Comment" => comment = val.to_string(),
                "Icon" => icon = val.to_string(),
                "Categories" => categories = val.to_string(),
                "Terminal" => terminal = val == "true",
                "NoDisplay" => no_display = val == "true",
                "Hidden" => hidden = val == "true",
                _ => {}
            }
        }
    }

    if name.is_empty() || exec.is_empty() || no_display || hidden {
        return None;
    }

    // strip field codes from Exec (%u, %U, %f, %F, etc.)
    let exec = exec
        .split_whitespace()
        .filter(|s| !s.starts_with('%'))
        .collect::<Vec<_>>()
        .join(" ");

    let cats: Vec<String> = categories
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Some(DesktopEntry {
        name,
        exec,
        comment,
        icon,
        categories: cats,
        terminal,
        path: path.clone(),
    })
}
