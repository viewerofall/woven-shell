//! XDG icon theme lookup and rasterization.
//!
//! Resolves .desktop Icon= names to actual file paths, loads PNG/SVG,
//! and caches RGBA bitmaps at the requested size.

use std::collections::HashMap;
use std::path::PathBuf;

const ICON_SIZE: u32 = 28; // rendered icon size in pixels

pub struct IconCache {
    /// icon name → RGBA pixels at ICON_SIZE × ICON_SIZE
    cache: HashMap<String, Option<Vec<u8>>>,
    /// theme search dirs, ordered by priority
    search_dirs: Vec<IconSearchDir>,
}

struct IconSearchDir {
    base: String,
    /// subdirs to check, in priority order (larger PNGs first, then scalable)
    subdirs: Vec<String>,
}

impl IconCache {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_default();

        // detect current icon theme from GTK settings
        let theme = detect_gtk_icon_theme(&home).unwrap_or_else(|| "hicolor".into());
        tracing::info!("launch: icon theme = {theme}");

        let mut search_dirs = Vec::new();

        // build search dirs: user → theme → hicolor → pixmaps
        let icon_bases = vec![
            format!("{home}/.local/share/icons"),
            "/usr/share/icons".into(),
        ];

        let themes: Vec<&str> = if theme == "hicolor" {
            vec!["hicolor"]
        } else {
            vec![&theme, "hicolor"]
        };

        for base in &icon_bases {
            for t in &themes {
                let theme_base = format!("{base}/{t}");
                if !std::path::Path::new(&theme_base).is_dir() { continue; }

                // prefer these size dirs in order
                let subdirs: Vec<String> = [
                    "48x48/apps", "64x64/apps", "32x32/apps", "128x128/apps",
                    "scalable/apps",
                    "48x48/categories", "32x32/categories",
                    "scalable/categories",
                    "48x48/mimetypes", "scalable/mimetypes",
                ].iter().map(|s| s.to_string()).collect();

                search_dirs.push(IconSearchDir {
                    base: theme_base,
                    subdirs,
                });
            }
        }

        // also check /usr/share/pixmaps as last resort
        search_dirs.push(IconSearchDir {
            base: "/usr/share/pixmaps".into(),
            subdirs: vec![String::new()], // flat directory
        });

        Self {
            cache: HashMap::with_capacity(256),
            search_dirs,
        }
    }

    /// Get RGBA pixels for an icon (ICON_SIZE × ICON_SIZE), or None if not found.
    pub fn get(&mut self, icon_name: &str) -> Option<&[u8]> {
        if icon_name.is_empty() {
            return None;
        }

        if !self.cache.contains_key(icon_name) {
            let result = self.load_icon(icon_name);
            self.cache.insert(icon_name.to_string(), result);
        }

        self.cache.get(icon_name).and_then(|v| v.as_deref())
    }

    pub fn icon_size(&self) -> u32 { ICON_SIZE }

    fn load_icon(&self, icon_name: &str) -> Option<Vec<u8>> {
        let path = self.resolve_path(icon_name)?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "png" | "jpg" | "jpeg" | "bmp" | "webp" => load_raster(&path, ICON_SIZE),
            "svg" | "svgz" => load_svg(&path, ICON_SIZE),
            "xpm" => None, // skip xpm
            _ => load_raster(&path, ICON_SIZE), // try as raster
        }
    }

    fn resolve_path(&self, icon_name: &str) -> Option<PathBuf> {
        // 1. absolute path — use directly
        if icon_name.starts_with('/') {
            let p = PathBuf::from(icon_name);
            if p.exists() { return Some(p); }
            return None;
        }

        // 2. has extension already (e.g. "firefox.png") — search as-is too
        let has_ext = icon_name.contains('.');

        // 3. search through theme dirs
        let extensions = ["png", "svg", "svgz", "xpm"];

        for dir in &self.search_dirs {
            for subdir in &dir.subdirs {
                let base = if subdir.is_empty() {
                    dir.base.clone()
                } else {
                    format!("{}/{subdir}", dir.base)
                };

                if has_ext {
                    let p = PathBuf::from(format!("{base}/{icon_name}"));
                    if p.exists() { return Some(p); }
                }

                for ext in &extensions {
                    let p = PathBuf::from(format!("{base}/{icon_name}.{ext}"));
                    if p.exists() { return Some(p); }
                }
            }
        }

        None
    }
}

fn load_raster(path: &PathBuf, size: u32) -> Option<Vec<u8>> {
    let img = image::open(path).ok()?;
    let img = img.resize_to_fill(size, size, image::imageops::FilterType::Triangle);
    Some(img.to_rgba8().into_raw())
}

fn load_svg(path: &PathBuf, size: u32) -> Option<Vec<u8>> {
    let data = std::fs::read(path).ok()?;

    // handle svgz (gzip compressed)
    let svg_data = if path.extension().and_then(|e| e.to_str()) == Some("svgz") {
        decompress_gzip(&data)?
    } else {
        data
    };

    let tree = resvg::usvg::Tree::from_data(&svg_data, &resvg::usvg::Options::default()).ok()?;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;

    let svg_size = tree.size();
    let scale_x = size as f32 / svg_size.width();
    let scale_y = size as f32 / svg_size.height();
    let scale = scale_x.min(scale_y);

    // center the icon
    let dx = (size as f32 - svg_size.width() * scale) / 2.0;
    let dy = (size as f32 - svg_size.height() * scale) / 2.0;

    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale)
        .post_translate(dx, dy);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // pixmap is already RGBA
    Some(pixmap.data().to_vec())
}

fn decompress_gzip(data: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

fn detect_gtk_icon_theme(home: &str) -> Option<String> {
    // try GTK3 settings first
    let paths = [
        format!("{home}/.config/gtk-3.0/settings.ini"),
        format!("{home}/.config/gtk-4.0/settings.ini"),
    ];
    for path in &paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                let line = line.trim();
                if let Some(val) = line.strip_prefix("gtk-icon-theme-name") {
                    let val = val.trim_start_matches(|c: char| c == '=' || c.is_whitespace());
                    let val = val.trim();
                    if !val.is_empty() { return Some(val.to_string()); }
                }
            }
        }
    }
    None
}
