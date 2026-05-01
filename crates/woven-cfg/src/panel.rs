//! Main panel: tab bar, content routing, shared drawing, click zone tracking.

use tiny_skia::*;
use crate::config::{AllConfigs, Bind};
use crate::widgets::{KeyCapture, TextInput, Toggle};

// ── Dimensions ────────────────────────────────────────────────────────────────

pub const DLG_W: u32 = 920;
pub const DLG_H: u32 = 600;
const TAB_H:     u32 = 44;
const BOT_H:     u32 = 48;
const CONTENT_H: u32 = DLG_H - TAB_H - BOT_H;
const PAD:       f32 = 16.0;
const ROW_H:     f32 = 36.0;

// ── Palette ───────────────────────────────────────────────────────────────────

const BG:        &str = "#0a0010";
const BG_DLG:    &str = "#0e0018";
const BG_CARD:   &str = "#160026";
const BG_HOVER:  &str = "#1e002e";
const ACCENT:    &str = "#c792ea";
const TEAL:      &str = "#00e5c8";
const FG:        &str = "#cdd6f4";
const DIM:       &str = "#6a508a";
const BORDER:    &str = "#2a1545";
const RED:       &str = "#f07178";
const TAB_ACTIVE: &str = "#1e0038";

// ── Tab ids ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab { Bar, Wall, Lock, Launch, Sway, Theme }

impl Tab {
    pub const ALL: &'static [Tab] = &[Tab::Bar, Tab::Wall, Tab::Lock, Tab::Launch, Tab::Sway, Tab::Theme];
    pub fn label(self) -> &'static str {
        match self { Tab::Bar => "Bar", Tab::Wall => "Wall", Tab::Lock => "Lock",
                     Tab::Launch => "Launch", Tab::Sway => "Sway", Tab::Theme => "Theme" }
    }
}

// ── Input events ──────────────────────────────────────────────────────────────

pub enum Input {
    Click(f32, f32),
    Scroll(f32),
    Char(char),
    Backspace,
    CtrlBackspace,
    Enter,
    Escape,
    Tab,
    Left, Right, Up, Down,
    KeyCombo { key: String },
}

// ── Click zone ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum ZoneAction {
    SelectTab(Tab),
    Close,
    Apply,
    // Sway tab
    SwayEdit(usize),
    SwayDelete(usize),
    SwayAdd,
    SwayEditField(SwayField),
    SwayEditSave,
    SwayEditCancel,
    // Bar tab
    BarModuleRemove { slot: BarSlot, idx: usize },
    BarModuleAdd(BarSlot),
    BarModulePicker(String),
    BarModulePickerClose,
    BarFieldFocus(BarField),
    BarToggle(BarToggleField),
    // Wall tab
    WallKindSelect(String),
    WallFieldFocus(WallField),
    // Lock tab
    LockProgramSelect(String),
    LockBgKindSelect(String),
    LockFieldFocus(LockField),
    LockToggle(LockToggleField),
    // Launch tab
    LaunchFieldFocus(LaunchField),
    LaunchToggle(LaunchToggleField),
    // Theme tab
    ThemeSelect(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum SwayField { Key, Label, Action }

#[derive(Clone, Debug, PartialEq)]
pub enum BarSlot { Left, Center, Right }

#[derive(Clone, Debug, PartialEq)]
pub enum BarField { Height, Position, Background, Foreground, Accent, Dim, FontSize }

#[derive(Clone, Debug, PartialEq)]
pub enum BarToggleField { Bubbles, WallpaperTheme }

#[derive(Clone, Debug, PartialEq)]
pub enum WallField { Path, Color, Dir, Interval, TransitionSecs }

#[derive(Clone, Debug, PartialEq)]
pub enum LockField { Dir, Path, BlurRadius, ClockFormat, DateFormat,
                     TextColor, AccentColor, ErrorColor, FadeInMs, LockProgram }

#[derive(Clone, Debug, PartialEq)]
pub enum LockToggleField { ShowClock, ShowDate, ShakeOnError }

#[derive(Clone, Debug, PartialEq)]
pub enum LaunchField { Width, MaxResults, Background, PanelBg, TextColor,
                       TextDim, AccentColor, SelectionColor, BorderColor }

#[derive(Clone, Debug, PartialEq)]
pub enum LaunchToggleField { Calculator, CommandRunner }

#[derive(Clone)]
pub struct Zone {
    pub x0: f32, pub y0: f32, pub x1: f32, pub y1: f32,
    pub action: ZoneAction,
}

// ── Sway edit state ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SwayEditState {
    pub idx:    Option<usize>,
    pub key:    KeyCapture,
    pub label:  TextInput,
    pub action: TextInput,
    pub focused: SwayField,
}

impl SwayEditState {
    pub fn new_add() -> Self {
        Self { idx: None, key: KeyCapture::new(""), label: TextInput::new(""),
               action: TextInput::new(""), focused: SwayField::Key }
    }
    pub fn from_bind(idx: usize, b: &Bind) -> Self {
        Self { idx: Some(idx), key: KeyCapture::new(&b.key),
               label: TextInput::new(&b.label), action: TextInput::new(&b.action),
               focused: SwayField::Key }
    }
}

// ── Panel state ───────────────────────────────────────────────────────────────

pub struct Panel {
    pub tab:          Tab,
    pub scroll_y:     f32,
    pub cfg:          AllConfigs,
    pub zones:        Vec<Zone>,
    pub dirty:        bool,
    pub should_close: bool,
    pub status_msg:   Option<(String, bool)>, // (text, is_error)
    pub status_timer: u32,

    // Sway tab
    pub sway_edit:  Option<SwayEditState>,
    pub sway_hover: Option<usize>,

    // Bar tab
    pub bar_module_picker: Option<BarSlot>,
    pub bar_focused: Option<BarField>,
    pub bar_inputs:  BarInputs,

    // Wall tab
    pub wall_focused: Option<WallField>,
    pub wall_inputs:  WallInputs,

    // Lock tab
    pub lock_focused: Option<LockField>,
    pub lock_inputs:  LockInputs,

    // Launch tab
    pub launch_focused: Option<LaunchField>,
    pub launch_inputs:  LaunchInputs,

    // Theme tab
    pub selected_theme: String,

    pub text_font: fontdue::Font,
}

// ── Per-tab input structs ─────────────────────────────────────────────────────

pub struct BarInputs {
    pub height:     TextInput,
    pub position:   TextInput,
    pub background: TextInput,
    pub foreground: TextInput,
    pub accent:     TextInput,
    pub dim:        TextInput,
    pub font_size:  TextInput,
    pub use_bubbles:         Toggle,
    pub wallpaper_theme:     Toggle,
}

pub struct WallInputs {
    pub path:           TextInput,
    pub color:          TextInput,
    pub dir:            TextInput,
    pub interval:       TextInput,
    pub transition_secs: TextInput,
    pub kind:           String,
}

pub struct LockInputs {
    pub lock_program: String,
    pub dir:          TextInput,
    pub path:         TextInput,
    pub blur_radius:  TextInput,
    pub clock_format: TextInput,
    pub date_format:  TextInput,
    pub text_color:   TextInput,
    pub accent_color: TextInput,
    pub error_color:  TextInput,
    pub fade_in_ms:   TextInput,
    pub bg_kind:      String,
    pub show_clock:   Toggle,
    pub show_date:    Toggle,
    pub shake:        Toggle,
}

pub struct LaunchInputs {
    pub width:         TextInput,
    pub max_results:   TextInput,
    pub background:    TextInput,
    pub panel_bg:      TextInput,
    pub text_color:    TextInput,
    pub text_dim:      TextInput,
    pub accent_color:  TextInput,
    pub selection:     TextInput,
    pub border_color:  TextInput,
    pub calculator:    Toggle,
    pub cmd_runner:    Toggle,
}

impl Panel {
    pub fn new() -> Self {
        let cfg = AllConfigs::load();
        let bar = &cfg.bar;
        let wall = &cfg.wall;
        let lock = &cfg.lock;
        let launch = &cfg.launch.launcher;

        let bar_inputs = BarInputs {
            height:     TextInput::new(bar.height.to_string()),
            position:   TextInput::new(&bar.position),
            background: TextInput::new(&bar.theme.background),
            foreground: TextInput::new(&bar.theme.foreground),
            accent:     TextInput::new(&bar.theme.accent),
            dim:        TextInput::new(&bar.theme.dim),
            font_size:  TextInput::new(bar.theme.font_size.to_string()),
            use_bubbles:     Toggle::new(bar.style == "bubbles"),
            wallpaper_theme: Toggle::new(bar.theme_source == "wallpaper"),
        };

        let wall_inputs = WallInputs {
            path:           TextInput::new(&wall.wallpaper.path),
            color:          TextInput::new(&wall.wallpaper.color),
            dir:            TextInput::new(&wall.wallpaper.dir),
            interval:       TextInput::new(wall.wallpaper.interval.to_string()),
            transition_secs: TextInput::new(wall.wallpaper.transition_secs.to_string()),
            kind:           wall.wallpaper.kind.clone(),
        };

        let lock_inputs = LockInputs {
            lock_program: lock.lock.lock_program.clone(),
            dir:          TextInput::new(&lock.background.dir),
            path:         TextInput::new(&lock.background.path),
            blur_radius:  TextInput::new(lock.lock.blur_radius.to_string()),
            clock_format: TextInput::new(&lock.lock.clock_format),
            date_format:  TextInput::new(&lock.lock.date_format),
            text_color:   TextInput::new(&lock.lock.text_color),
            accent_color: TextInput::new(&lock.lock.accent_color),
            error_color:  TextInput::new(&lock.lock.error_color),
            fade_in_ms:   TextInput::new(lock.lock.fade_in_ms.to_string()),
            bg_kind:      lock.background.kind.clone(),
            show_clock:   Toggle::new(lock.lock.show_clock),
            show_date:    Toggle::new(lock.lock.show_date),
            shake:        Toggle::new(lock.lock.shake_on_error),
        };

        let launch_inputs = LaunchInputs {
            width:        TextInput::new(launch.width.to_string()),
            max_results:  TextInput::new(launch.max_results.to_string()),
            background:   TextInput::new(&launch.background),
            panel_bg:     TextInput::new(&launch.panel_background),
            text_color:   TextInput::new(&launch.text_color),
            text_dim:     TextInput::new(&launch.text_dim),
            accent_color: TextInput::new(&launch.accent_color),
            selection:    TextInput::new(&launch.selection_color),
            border_color: TextInput::new(&launch.border_color),
            calculator:   Toggle::new(launch.calculator),
            cmd_runner:   Toggle::new(launch.command_runner),
        };

        let text_font = load_font();

        Self {
            tab: Tab::Sway, scroll_y: 0.0, cfg, zones: vec![], dirty: true,
            should_close: false, status_msg: None, status_timer: 0,
            sway_edit: None, sway_hover: None,
            bar_module_picker: None, bar_focused: None, bar_inputs,
            wall_focused: None, wall_inputs,
            lock_focused: None, lock_inputs,
            launch_focused: None, launch_inputs,
            selected_theme: "catppuccin".to_string(),
            text_font,
        }
    }

    // ── Input dispatch ────────────────────────────────────────────────────────

    pub fn handle(&mut self, input: Input) {
        match input {
            Input::Click(x, y) => self.handle_click(x, y),
            Input::Scroll(dy)  => {
                self.scroll_y = (self.scroll_y + dy * 30.0)
                    .max(0.0).min(self.max_scroll());
                self.dirty = true;
            }
            Input::Char(ch)         => self.handle_char(ch),
            Input::Backspace        => self.handle_backspace(false),
            Input::CtrlBackspace    => self.handle_backspace(true),
            Input::Enter            => self.handle_enter(),
            Input::Escape           => self.handle_escape(),
            Input::Tab              => self.handle_tab_key(),
            Input::Left | Input::Right | Input::Up | Input::Down => {}
            Input::KeyCombo { key } => self.handle_key_combo(key),
        }
    }

    fn handle_click(&mut self, x: f32, y: f32) {
        let zones = std::mem::take(&mut self.zones);
        for z in &zones {
            if x >= z.x0 && x <= z.x1 && y >= z.y0 && y <= z.y1 {
                let action = z.action.clone();
                self.zones = zones;
                self.dispatch_action(action);
                return;
            }
        }
        self.zones = zones;
        self.unfocus_all();
    }

    fn unfocus_all(&mut self) {
        self.bar_focused    = None;
        self.wall_focused   = None;
        self.lock_focused   = None;
        self.launch_focused = None;
        if let Some(ref mut e) = self.sway_edit {
            e.key.capturing = false;
        }
        self.dirty = true;
    }

    fn dispatch_action(&mut self, action: ZoneAction) {
        self.unfocus_all();
        match action {
            ZoneAction::SelectTab(t) => {
                self.tab = t;
                self.scroll_y = 0.0;
                self.sway_edit = None;
                self.bar_module_picker = None;
            }
            ZoneAction::Close => { self.should_close = true; return; }
            ZoneAction::Apply => self.apply(),

            // Sway tab
            ZoneAction::SwayEdit(i)   => {
                let b = self.cfg.keybinds.binds[i].clone();
                self.sway_edit = Some(SwayEditState::from_bind(i, &b));
            }
            ZoneAction::SwayDelete(i) => {
                self.cfg.keybinds.binds.remove(i);
            }
            ZoneAction::SwayAdd       => {
                self.sway_edit = Some(SwayEditState::new_add());
            }
            ZoneAction::SwayEditField(f) => {
                if let Some(ref mut e) = self.sway_edit {
                    e.focused = f.clone();
                    if f == SwayField::Key { e.key.capturing = true; }
                }
            }
            ZoneAction::SwayEditSave   => self.sway_edit_save(),
            ZoneAction::SwayEditCancel => { self.sway_edit = None; }

            // Bar tab
            ZoneAction::BarModuleRemove { slot, idx } => {
                let list = match slot {
                    BarSlot::Left   => &mut self.cfg.bar.modules.left,
                    BarSlot::Center => &mut self.cfg.bar.modules.center,
                    BarSlot::Right  => &mut self.cfg.bar.modules.right,
                };
                if idx < list.len() { list.remove(idx); }
            }
            ZoneAction::BarModuleAdd(slot)    => {
                self.bar_module_picker = Some(slot);
            }
            ZoneAction::BarModulePicker(name) => {
                if let Some(ref slot) = self.bar_module_picker.clone() {
                    let list = match slot {
                        BarSlot::Left   => &mut self.cfg.bar.modules.left,
                        BarSlot::Center => &mut self.cfg.bar.modules.center,
                        BarSlot::Right  => &mut self.cfg.bar.modules.right,
                    };
                    list.push(name);
                }
                self.bar_module_picker = None;
            }
            ZoneAction::BarModulePickerClose  => { self.bar_module_picker = None; }
            ZoneAction::BarFieldFocus(f)      => { self.bar_focused = Some(f); }
            ZoneAction::BarToggle(f) => {
                match f {
                    BarToggleField::Bubbles        => self.bar_inputs.use_bubbles.flip(),
                    BarToggleField::WallpaperTheme => self.bar_inputs.wallpaper_theme.flip(),
                }
            }

            // Wall tab
            ZoneAction::WallKindSelect(k)  => { self.wall_inputs.kind = k; }
            ZoneAction::WallFieldFocus(f)  => { self.wall_focused = Some(f); }

            // Lock tab
            ZoneAction::LockProgramSelect(p) => { self.lock_inputs.lock_program = p; }
            ZoneAction::LockBgKindSelect(k) => { self.lock_inputs.bg_kind = k; }
            ZoneAction::LockFieldFocus(f)   => { self.lock_focused = Some(f); }
            ZoneAction::LockToggle(f) => {
                match f {
                    LockToggleField::ShowClock    => self.lock_inputs.show_clock.flip(),
                    LockToggleField::ShowDate     => self.lock_inputs.show_date.flip(),
                    LockToggleField::ShakeOnError => self.lock_inputs.shake.flip(),
                }
            }

            // Launch tab
            ZoneAction::LaunchFieldFocus(f) => { self.launch_focused = Some(f); }
            ZoneAction::LaunchToggle(f) => {
                match f {
                    LaunchToggleField::Calculator  => self.launch_inputs.calculator.flip(),
                    LaunchToggleField::CommandRunner => self.launch_inputs.cmd_runner.flip(),
                }
            }

            // Theme tab
            ZoneAction::ThemeSelect(name) => { self.selected_theme = name; }
        }
        self.dirty = true;
    }

    fn active_text_input(&mut self) -> Option<&mut TextInput> {
        if let Some(ref mut e) = self.sway_edit {
            if e.key.capturing { return None; }
            return match e.focused {
                SwayField::Label  => Some(&mut e.label),
                SwayField::Action => Some(&mut e.action),
                SwayField::Key    => None,
            };
        }
        if let Some(ref f) = self.bar_focused.clone() {
            return Some(match f {
                BarField::Height     => &mut self.bar_inputs.height,
                BarField::Position   => &mut self.bar_inputs.position,
                BarField::Background => &mut self.bar_inputs.background,
                BarField::Foreground => &mut self.bar_inputs.foreground,
                BarField::Accent     => &mut self.bar_inputs.accent,
                BarField::Dim        => &mut self.bar_inputs.dim,
                BarField::FontSize   => &mut self.bar_inputs.font_size,
            });
        }
        if let Some(ref f) = self.wall_focused.clone() {
            return Some(match f {
                WallField::Path           => &mut self.wall_inputs.path,
                WallField::Color          => &mut self.wall_inputs.color,
                WallField::Dir            => &mut self.wall_inputs.dir,
                WallField::Interval       => &mut self.wall_inputs.interval,
                WallField::TransitionSecs => &mut self.wall_inputs.transition_secs,
            });
        }
        if let Some(ref f) = self.lock_focused.clone() {
            return match f {
                LockField::Dir        => Some(&mut self.lock_inputs.dir),
                LockField::Path       => Some(&mut self.lock_inputs.path),
                LockField::BlurRadius => Some(&mut self.lock_inputs.blur_radius),
                LockField::ClockFormat => Some(&mut self.lock_inputs.clock_format),
                LockField::DateFormat  => Some(&mut self.lock_inputs.date_format),
                LockField::TextColor   => Some(&mut self.lock_inputs.text_color),
                LockField::AccentColor => Some(&mut self.lock_inputs.accent_color),
                LockField::ErrorColor  => Some(&mut self.lock_inputs.error_color),
                LockField::FadeInMs    => Some(&mut self.lock_inputs.fade_in_ms),
                LockField::LockProgram => None,
            };
        }
        if let Some(ref f) = self.launch_focused.clone() {
            return Some(match f {
                LaunchField::Width        => &mut self.launch_inputs.width,
                LaunchField::MaxResults   => &mut self.launch_inputs.max_results,
                LaunchField::Background   => &mut self.launch_inputs.background,
                LaunchField::PanelBg      => &mut self.launch_inputs.panel_bg,
                LaunchField::TextColor    => &mut self.launch_inputs.text_color,
                LaunchField::TextDim      => &mut self.launch_inputs.text_dim,
                LaunchField::AccentColor  => &mut self.launch_inputs.accent_color,
                LaunchField::SelectionColor => &mut self.launch_inputs.selection,
                LaunchField::BorderColor  => &mut self.launch_inputs.border_color,
            });
        }
        None
    }

    fn handle_char(&mut self, ch: char) {
        if let Some(inp) = self.active_text_input() {
            inp.push(ch);
            self.dirty = true;
        }
    }

    fn handle_backspace(&mut self, word: bool) {
        if let Some(inp) = self.active_text_input() {
            if word { inp.delete_word(); } else { inp.backspace(); }
            self.dirty = true;
        }
    }

    fn handle_enter(&mut self) {
        if self.sway_edit.is_some() {
            self.sway_edit_save();
        }
    }

    fn handle_escape(&mut self) {
        if let Some(ref mut e) = self.sway_edit {
            if e.key.capturing { e.key.capturing = false; self.dirty = true; return; }
        }
        if self.sway_edit.is_some() { self.sway_edit = None; self.dirty = true; return; }
        if self.bar_module_picker.is_some() { self.bar_module_picker = None; self.dirty = true; return; }
        self.unfocus_all();
    }

    fn handle_tab_key(&mut self) {
        // Cycle focus within sway edit fields
        if let Some(ref mut e) = self.sway_edit {
            e.focused = match e.focused {
                SwayField::Key    => { e.key.capturing = false; SwayField::Label }
                SwayField::Label  => SwayField::Action,
                SwayField::Action => { e.key.capturing = true; SwayField::Key }
            };
            self.dirty = true;
        }
    }

    fn handle_key_combo(&mut self, key: String) {
        if let Some(ref mut e) = self.sway_edit {
            if e.key.capturing {
                e.key.value    = key;
                e.key.capturing = false;
                e.focused      = SwayField::Label;
                self.dirty     = true;
            }
        }
    }

    fn sway_edit_save(&mut self) {
        if let Some(ref e) = self.sway_edit.clone() {
            if e.key.value.is_empty() || e.action.value.is_empty() {
                self.set_status("Key and action cannot be empty", true);
                return;
            }
            let bind = Bind {
                category: match e.idx {
                    Some(i) => self.cfg.keybinds.binds[i].category.clone(),
                    None    => "apps".into(),
                },
                key:    e.key.value.clone(),
                label:  e.label.value.clone(),
                action: e.action.value.clone(),
            };
            match e.idx {
                Some(i) => self.cfg.keybinds.binds[i] = bind,
                None    => self.cfg.keybinds.binds.push(bind),
            }
            self.sway_edit = None;
        }
        self.dirty = true;
    }

    fn apply(&mut self) {
        // Flush text input values back into cfg structs
        self.flush_inputs();

        // Handle theme selection
        if self.tab == Tab::Theme {
            self.apply_theme();
        }

        let result = match self.tab {
            Tab::Bar    => self.cfg.save_bar(),
            Tab::Wall   => self.cfg.save_wall(),
            Tab::Lock   => self.cfg.save_lock(),
            Tab::Launch => self.cfg.save_launch(),
            Tab::Sway   => self.cfg.save_keybinds(),
            Tab::Theme  => self.cfg.save_bar(),
        };
        match result {
            Ok(_)  => self.set_status("Saved.", false),
            Err(e) => self.set_status(&format!("Error: {e}"), true),
        }
    }

    fn apply_theme(&mut self) {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let theme_path = format!("{}/.config/woven-shell/themes/{}.toml", home, self.selected_theme);

        if let Ok(content) = std::fs::read_to_string(&theme_path) {
            if let Ok(parsed) = toml::from_str::<toml::Table>(&content) {
                if let Some(theme) = parsed.get("theme").and_then(|t| t.as_table()) {
                    if let Some(bg) = theme.get("background").and_then(|v| v.as_str()) {
                        self.cfg.bar.theme.background = bg.to_string();
                    }
                    if let Some(fg) = theme.get("foreground").and_then(|v| v.as_str()) {
                        self.cfg.bar.theme.foreground = fg.to_string();
                    }
                    if let Some(acc) = theme.get("accent").and_then(|v| v.as_str()) {
                        self.cfg.bar.theme.accent = acc.to_string();
                    }
                    if let Some(dim) = theme.get("dim").and_then(|v| v.as_str()) {
                        self.cfg.bar.theme.dim = dim.to_string();
                    }
                }
            }
        }
    }

    fn flush_inputs(&mut self) {
        let b = &mut self.cfg.bar;
        if let Ok(v) = self.bar_inputs.height.value.trim().parse::<u32>()  { b.height = v; }
        b.position = self.bar_inputs.position.value.trim().to_string();
        b.theme.background = self.bar_inputs.background.value.trim().to_string();
        b.theme.foreground = self.bar_inputs.foreground.value.trim().to_string();
        b.theme.accent     = self.bar_inputs.accent.value.trim().to_string();
        b.theme.dim        = self.bar_inputs.dim.value.trim().to_string();
        if let Ok(v) = self.bar_inputs.font_size.value.trim().parse::<f32>() { b.theme.font_size = v; }
        b.theme_source = if self.bar_inputs.wallpaper_theme.value { "wallpaper" } else { "config" }.into();
        b.style = if self.bar_inputs.use_bubbles.value { "bubbles" } else { "solid" }.into();

        let w = &mut self.cfg.wall.wallpaper;
        w.kind   = self.wall_inputs.kind.clone();
        w.path   = self.wall_inputs.path.value.trim().to_string();
        w.color  = self.wall_inputs.color.value.trim().to_string();
        w.dir    = self.wall_inputs.dir.value.trim().to_string();
        if let Ok(v) = self.wall_inputs.interval.value.trim().parse::<u32>()     { w.interval = v; }
        if let Ok(v) = self.wall_inputs.transition_secs.value.trim().parse::<f32>() { w.transition_secs = v; }

        let l = &mut self.cfg.lock;
        l.background.kind = self.lock_inputs.bg_kind.clone();
        l.background.dir  = self.lock_inputs.dir.value.trim().to_string();
        l.background.path = self.lock_inputs.path.value.trim().to_string();
        l.lock.lock_program = self.lock_inputs.lock_program.clone();
        if let Ok(v) = self.lock_inputs.blur_radius.value.trim().parse::<u32>()  { l.lock.blur_radius = v; }
        if let Ok(v) = self.lock_inputs.fade_in_ms.value.trim().parse::<u32>()   { l.lock.fade_in_ms = v; l.lock.fade_out_ms = v; }
        l.lock.clock_format   = self.lock_inputs.clock_format.value.trim().to_string();
        l.lock.date_format    = self.lock_inputs.date_format.value.trim().to_string();
        l.lock.text_color     = self.lock_inputs.text_color.value.trim().to_string();
        l.lock.accent_color   = self.lock_inputs.accent_color.value.trim().to_string();
        l.lock.error_color    = self.lock_inputs.error_color.value.trim().to_string();
        l.lock.show_clock     = self.lock_inputs.show_clock.value;
        l.lock.show_date      = self.lock_inputs.show_date.value;
        l.lock.shake_on_error = self.lock_inputs.shake.value;

        let la = &mut self.cfg.launch.launcher;
        if let Ok(v) = self.launch_inputs.width.value.trim().parse::<u32>()      { la.width = v; }
        if let Ok(v) = self.launch_inputs.max_results.value.trim().parse::<u32>() { la.max_results = v; }
        la.background       = self.launch_inputs.background.value.trim().to_string();
        la.panel_background = self.launch_inputs.panel_bg.value.trim().to_string();
        la.text_color       = self.launch_inputs.text_color.value.trim().to_string();
        la.text_dim         = self.launch_inputs.text_dim.value.trim().to_string();
        la.accent_color     = self.launch_inputs.accent_color.value.trim().to_string();
        la.selection_color  = self.launch_inputs.selection.value.trim().to_string();
        la.border_color     = self.launch_inputs.border_color.value.trim().to_string();
        la.calculator       = self.launch_inputs.calculator.value;
        la.command_runner   = self.launch_inputs.cmd_runner.value;
    }

    fn set_status(&mut self, msg: &str, err: bool) {
        self.status_msg   = Some((msg.to_string(), err));
        self.status_timer = 120;
        self.dirty        = true;
    }

    pub fn tick_status(&mut self) {
        if self.status_timer > 0 {
            self.status_timer -= 1;
            if self.status_timer == 0 { self.status_msg = None; self.dirty = true; }
        }
    }

    fn max_scroll(&self) -> f32 {
        match self.tab {
            Tab::Sway => {
                let rows = self.cfg.keybinds.binds.len() as f32 * ROW_H + 60.0;
                (rows - CONTENT_H as f32).max(0.0)
            }
            _ => 500.0,
        }
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    pub fn render(&mut self, screen_w: u32, screen_h: u32) -> Vec<u8> {
        self.zones.clear();
        let sw = screen_w as f32;
        let sh = screen_h as f32;
        let dw = DLG_W as f32;
        let dh = DLG_H as f32;
        let ox = ((sw - dw) / 2.0).max(0.0).floor();
        let oy = ((sh - dh) / 2.0).max(0.0).floor();

        let mut pm = Pixmap::new(screen_w, screen_h).unwrap();
        // semi-transparent overlay
        pm.fill(Color::from_rgba8(0, 0, 0, 160));

        // dialog background
        fill_rrect(&mut pm, ox, oy, dw, dh, 12.0, BG_DLG);
        // dialog border
        stroke_rrect(&mut pm, ox, oy, dw, dh, 12.0, BORDER);

        // ── Tab bar ───────────────────────────────────────────────────────────
        let tab_y  = oy + 8.0;
        let tab_h  = TAB_H as f32;
        let mut tx = ox + PAD;
        for &t in Tab::ALL {
            let lbl  = t.label();
            let lw   = measure(&self.text_font, lbl, 13.0);
            let tw   = lw + 28.0;
            let is_a = self.tab == t;
            let bg   = if is_a { TAB_ACTIVE } else { "" };
            let fg_c = if is_a { ACCENT } else { DIM };
            if is_a { fill_rrect(&mut pm, tx, tab_y + 4.0, tw, tab_h - 8.0, 8.0, bg); }
            draw_text(&mut pm, &self.text_font, lbl, tx + 14.0, tab_y + 14.0, 13.0, fg_c);
            self.zones.push(Zone { x0: tx, y0: tab_y, x1: tx + tw, y1: tab_y + tab_h,
                                   action: ZoneAction::SelectTab(t) });
            tx += tw + 6.0;
        }

        // ── Content area (clipped) ────────────────────────────────────────────
        let cy = oy + TAB_H as f32 + 4.0;
        let ch = CONTENT_H as f32;
        let cx = ox + 4.0;
        let cw = dw - 8.0;

        // Render tab content into temporary pixmap, then blit
        let inner_h = (ch + self.scroll_y + 100.0) as u32;
        let zones_before = self.zones.len();
        if let Some(mut inner) = Pixmap::new(cw as u32, inner_h.max(1)) {
            inner.fill(hex(BG_DLG));
            match self.tab {
                Tab::Bar    => crate::tabs::bar::render(self, &mut inner, cw),
                Tab::Wall   => crate::tabs::wall::render(self, &mut inner, cw),
                Tab::Lock   => crate::tabs::lock::render(self, &mut inner, cw),
                Tab::Launch => crate::tabs::launch::render(self, &mut inner, cw),
                Tab::Sway   => crate::tabs::sway::render(self, &mut inner, cw),
                Tab::Theme  => crate::tabs::theme::render(self, &mut inner, cw),
            }
            // Blit inner → outer with scroll clipping
            let src_y = self.scroll_y as i32;
            let rows_to_blit = (ch as i32).min(inner_h as i32 - src_y).max(0) as u32;
            if rows_to_blit > 0 {
                let dst = pm.pixels_mut();
                let src = inner.pixels();
                let dw_u = screen_w as usize;
                let sw_u = cw as usize;
                for row in 0..rows_to_blit as usize {
                    let sy = (src_y as usize + row) * sw_u;
                    let dy = ((cy as usize + row) * dw_u) + cx as usize;
                    if sy + sw_u <= src.len() && dy + sw_u <= dst.len() {
                        dst[dy..dy + sw_u].copy_from_slice(&src[sy..sy + sw_u]);
                    }
                }
            }
        }

        // Translate content zones from inner-pixmap coords → screen coords,
        // then drop any that are fully outside the visible content viewport.
        let zone_ox = cx;
        let zone_oy = cy - self.scroll_y;
        let content_y0 = cy;
        let content_y1 = cy + ch;
        let mut i = zones_before;
        while i < self.zones.len() {
            let z = &mut self.zones[i];
            z.x0 += zone_ox;
            z.x1 += zone_ox;
            z.y0 += zone_oy;
            z.y1 += zone_oy;
            // Remove zones scrolled entirely out of the visible area
            if z.y1 < content_y0 || z.y0 > content_y1 {
                self.zones.remove(i);
            } else {
                i += 1;
            }
        }

        // ── Bottom bar ────────────────────────────────────────────────────────
        let by = oy + dh - BOT_H as f32;
        fill_rect(&mut pm, ox, by, dw, 1.0, BORDER);

        // Status message
        if let Some((ref msg, is_err)) = self.status_msg {
            let col = if is_err { RED } else { TEAL };
            draw_text(&mut pm, &self.text_font, msg, ox + PAD, by + 15.0, 12.0, col);
        }

        // Apply button
        let apply_w = 80.0f32;
        let apply_x = ox + dw - apply_w * 2.0 - PAD * 2.0 - 8.0;
        fill_rrect(&mut pm, apply_x, by + 8.0, apply_w, 32.0, 8.0, ACCENT);
        let aw = measure(&self.text_font, "Apply", 13.0);
        draw_text(&mut pm, &self.text_font, "Apply",
                  apply_x + (apply_w - aw) / 2.0, by + 14.0, 13.0, BG);
        self.zones.push(Zone { x0: apply_x, y0: by + 8.0,
                               x1: apply_x + apply_w, y1: by + 40.0,
                               action: ZoneAction::Apply });

        // Close button
        let close_x = ox + dw - apply_w - PAD;
        fill_rrect(&mut pm, close_x, by + 8.0, apply_w, 32.0, 8.0, BORDER);
        let cw2 = measure(&self.text_font, "Close", 13.0);
        draw_text(&mut pm, &self.text_font, "Close",
                  close_x + (apply_w - cw2) / 2.0, by + 14.0, 13.0, FG);
        self.zones.push(Zone { x0: close_x, y0: by + 8.0,
                               x1: close_x + apply_w, y1: by + 40.0,
                               action: ZoneAction::Close });

        // BGRA conversion
        let data = pm.data();
        let mut out = Vec::with_capacity(data.len());
        for chunk in data.chunks_exact(4) {
            out.push(chunk[2]);
            out.push(chunk[1]);
            out.push(chunk[0]);
            out.push(chunk[3]);
        }
        out
    }

    // ── Zone registration (called from tab renders, adjusted for content pos) ─

    pub fn add_zone(&mut self, screen_x: f32, screen_y: f32,
                    w: f32, h: f32, action: ZoneAction) {
        self.zones.push(Zone { x0: screen_x, y0: screen_y,
                               x1: screen_x + w, y1: screen_y + h, action });
    }
}

// ── Drawing primitives ────────────────────────────────────────────────────────

pub fn hex(s: &str) -> Color {
    if s.is_empty() { return Color::TRANSPARENT; }
    let s = s.trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).unwrap_or(0xFFFFFF);
    Color::from_rgba8((v >> 16) as u8, ((v >> 8) & 0xFF) as u8, (v & 0xFF) as u8, 255)
}

pub fn hex_a(s: &str, a: u8) -> Color {
    let mut c = hex(s);
    c.set_alpha(a as f32 / 255.0);
    c
}

fn paint(color: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    p
}

pub fn fill_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, col: &str) {
    if w <= 0.0 || h <= 0.0 { return; }
    let Some(rect) = Rect::from_xywh(x, y, w, h) else { return };
    let mut pa = paint(hex(col));
    pa.anti_alias = false;
    pm.fill_rect(rect, &pa, Transform::identity(), None);
}

pub fn fill_rrect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, col: &str) {
    if w <= 0.0 || h <= 0.0 { return; }
    if r <= 0.0 || w < r * 2.0 || h < r * 2.0 { fill_rect(pm, x, y, w, h, col); return; }
    let c = hex(col);
    if c.alpha() == 0.0 { return; }
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(c), FillRule::Winding, Transform::identity(), None);
    }
}

pub fn stroke_rrect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, col: &str) {
    let c = hex(col);
    let mut pb = PathBuilder::new();
    let off = 0.5f32;
    pb.move_to(x + r + off, y + off);
    pb.line_to(x + w - r - off, y + off);
    pb.quad_to(x + w - off, y + off, x + w - off, y + r + off);
    pb.line_to(x + w - off, y + h - r - off);
    pb.quad_to(x + w - off, y + h - off, x + w - r - off, y + h - off);
    pb.line_to(x + r + off, y + h - off);
    pb.quad_to(x + off, y + h - off, x + off, y + h - r - off);
    pb.line_to(x + off, y + r + off);
    pb.quad_to(x + off, y + off, x + r + off, y + off);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut stroke = Stroke::default();
        stroke.width = 1.0;
        pm.stroke_path(&path, &paint(c), &stroke, Transform::identity(), None);
    }
}

pub fn draw_text(pm: &mut Pixmap, font: &fontdue::Font,
                 text: &str, x: f32, y: f32, size: f32, col: &str) {
    let color = hex(col);
    let r = (color.red()   * 255.0) as u8;
    let g = (color.green() * 255.0) as u8;
    let b = (color.blue()  * 255.0) as u8;
    let a_base = (color.alpha() * 255.0) as u8;
    if a_base == 0 { return; }

    let pw = pm.width()  as i32;
    let ph = pm.height() as i32;
    let mut cx = x;

    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size);
        if metrics.width == 0 { cx += metrics.advance_width; continue; }
        let gx = (cx + metrics.xmin as f32).round() as i32;
        let gy = (y  + size - metrics.height as f32 - metrics.ymin as f32).round() as i32;
        let pixels = pm.pixels_mut();
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let coverage = bitmap[row * metrics.width + col];
                if coverage == 0 { continue; }
                let px = gx + col as i32;
                let py = gy + row as i32;
                if px < 0 || py < 0 || px >= pw || py >= ph { continue; }
                let idx   = (py * pw + px) as usize;
                let dst   = &mut pixels[idx];
                let src_a = (coverage as u16 * a_base as u16 / 255) as u8;
                let inv_a = 255u16 - src_a as u16;
                let dr = (r as u16 * src_a as u16 / 255 + dst.red()   as u16 * inv_a / 255) as u8;
                let dg = (g as u16 * src_a as u16 / 255 + dst.green() as u16 * inv_a / 255) as u8;
                let db = (b as u16 * src_a as u16 / 255 + dst.blue()  as u16 * inv_a / 255) as u8;
                let da = (src_a as u16 + dst.alpha() as u16 * inv_a / 255).min(255) as u8;
                *dst = PremultipliedColorU8::from_rgba(dr, dg, db, da).unwrap_or(*dst);
            }
        }
        cx += metrics.advance_width;
    }
}

pub fn measure(font: &fontdue::Font, text: &str, size: f32) -> f32 {
    text.chars().map(|c| font.metrics(c, size).advance_width).sum()
}

pub fn draw_field(pm: &mut Pixmap, font: &fontdue::Font,
                  label: &str, val: &str, focused: bool,
                  x: f32, y: f32, w: f32, h: f32) {
    let bg = if focused { "#1e0038" } else { BG_CARD };
    let bd = if focused { ACCENT } else { BORDER };
    fill_rrect(pm, x, y, w, h, 6.0, bg);
    stroke_rrect(pm, x, y, w, h, 6.0, bd);
    draw_text(pm, font, label, x + 8.0, y + 4.0, 10.0, DIM);
    draw_text(pm, font, val, x + 8.0, y + 16.0, 12.0, FG);
    if focused {
        // cursor
        let cx = x + 8.0 + measure(font, val, 12.0);
        fill_rect(pm, cx, y + 16.0, 1.5, 13.0, ACCENT);
    }
}

pub fn draw_toggle(pm: &mut Pixmap, x: f32, y: f32, on: bool) {
    let bg = if on { ACCENT } else { BORDER };
    fill_rrect(pm, x, y, 36.0, 18.0, 9.0, bg);
    let kx = if on { x + 20.0 } else { x + 2.0 };
    fill_rrect(pm, kx, y + 2.0, 14.0, 14.0, 7.0, FG);
}

pub fn draw_pill(pm: &mut Pixmap, font: &fontdue::Font,
                 label: &str, x: f32, y: f32, h: f32,
                 fg_col: &str, bg_col: &str) -> f32 {
    let tw = measure(font, label, 11.5) + 16.0;
    fill_rrect(pm, x, y, tw, h, h / 2.0, bg_col);
    draw_text(pm, font, label, x + 8.0, y + (h - 11.5) / 2.0, 11.5, fg_col);
    tw
}

pub fn cat_color(cat: &str) -> &'static str {
    match cat {
        "core"        => "#c792ea",
        "apps"        => "#00e5c8",
        "woven"       => "#82aaff",
        "focus"       => "#f07178",
        "move"        => "#ffcb6b",
        "layout"      => "#c3e88d",
        "workspaces"  => "#f78c6c",
        "media"       => "#89ddff",
        "screenshots" => "#b2ccd6",
        _             => "#cdd6f4",
    }
}

fn load_font() -> fontdue::Font {
    for p in &[
        "/usr/share/fonts/TTF/Inconsolata-Regular.ttf",
        "/usr/share/fonts/TTF/Inconsolata.ttf",
        "/usr/share/fonts/truetype/inconsolata/Inconsolata-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ] {
        if let Ok(data) = std::fs::read(p) {
            if let Ok(f) = fontdue::Font::from_bytes(data.as_slice(), fontdue::FontSettings::default()) {
                return f;
            }
        }
    }
    panic!("woven-cfg: no usable font found");
}
