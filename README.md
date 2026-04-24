# woven-shell

A complete Wayland shell for Sway. Custom bar, launcher, lock screen, wallpaper daemon, power menu, control center, config manager, OSD, screenshots, workspace switcher, and session daemon — all in Rust.

Theme: `#0a0010` bg · `#c792ea` accent · `#00e5c8` teal

---

## Quick Install

### Option 1: Pre-built Binaries (Recommended)

Download and install pre-built binaries from the latest release:

```bash
curl -fsSL https://raw.githubusercontent.com/viewerofall/woven-shell/main/get.sh | sh
```

Install a single component:

```bash
curl -fsSL https://raw.githubusercontent.com/viewerofall/woven-shell/main/get.sh | sh -s -- --install woven-osd
```

Install everything without prompts:

```bash
curl -fsSL https://raw.githubusercontent.com/viewerofall/woven-shell/main/get.sh | sh -s -- --all
```

### Option 2: Build from Source Locally

Clone the repo and build with `install.sh`:

```bash
git clone https://github.com/viewerofall/woven-shell
cd woven-shell
./install.sh
```

Or install a single component:

```bash
./install.sh --install woven-screenshot
```

Or install everything without prompting:

```bash
./install.sh --all
```

Or build a release package for distribution:

```bash
./install.sh --package
```

This creates `dist/woven-shell.tar.gz` with all binaries, configs, and the `get.sh` installer.

---

## Components

| Binary | What it does |
|---|---|
| `woven-bar` | Persistent top bar — workspaces, window title, clock, battery, audio, network |
| `woven-power` | Power menu overlay — shutdown, reboot, suspend, logout |
| `woven-cc` | Control center — quick toggles and sliders |
| `woven-launch` | App launcher (rofi/wofi replacement) |
| `woven-lock` | Lock screen with PAM auth |
| `woven-wall` | Wallpaper daemon — static image or gradient |
| `woven-pick` | Color/item picker utility |
| `woven-cfg` | Config manager GUI — edit all woven-shell settings and Sway keybinds live |
| `woven-osd` | On-screen display daemon — volume (with device name), brightness, media info |
| `woven-screenshot` | Screenshot utility — full screen, area, or window mode (saves to ~/Pictures/Screenshots) |
| `woven-session` | Session daemon — IPC hub, battery/media state tracking, DBus integration |
| `woven-switch` | Workspace switcher — cycle through Sway workspaces |

---

## Requirements

- **Sway** compositor — wlr-layer-shell, Sway IPC
- Rust stable toolchain (for building from source)
- `pam` development headers (for woven-lock)
- Runtime: `wpctl`, `brightnessctl`, `playerctl`, `swaync`, `slurp`, `grim` (for woven-screenshot)

---

## Installation Breakdown

### Scripts

- **`get.sh`** — Remote installer. Downloads pre-built binaries from GitHub releases. Use this for quick installation as an end-user.
- **`install.sh`** — Local installer and packaging tool. Builds from source, installs locally, and creates release tarballs. Use this for development or if you want to build from source.

### Manual Installation

```bash
git clone https://github.com/viewerofall/woven-shell
cd woven-shell
cargo build --release
cp target/release/woven-{bar,power,cc,launch,lock,wall,pick,cfg,osd,screenshot,session,switch} ~/.local/bin/
mkdir -p ~/.config/woven-shell && cp config/*.toml ~/.config/woven-shell/
```

---

## Sway Configuration

Add to your `~/.config/sway/config`:

```
# Autostart
exec_always --no-startup-id ~/.local/bin/woven-wall
exec_always --no-startup-id ~/.local/bin/woven-bar
exec_always --no-startup-id /usr/bin/swaync
exec ~/.local/bin/woven-session
exec ~/.local/bin/woven-osd

# Keybinds
bindsym $mod+Return      exec kitty
bindsym $mod+d           exec ~/.local/bin/woven-launch
bindsym $mod+q           kill
bindsym $mod+Shift+p     exec ~/.local/bin/woven-power
bindsym $mod+o           exec ~/.local/bin/woven-cfg
bindsym $mod+Escape      exec ~/.local/bin/woven-lock
bindsym $mod+Shift+s     exec ~/.local/bin/woven-screenshot full
bindsym $mod+s           exec ~/.local/bin/woven-screenshot area
bindsym $mod+Tab         exec ~/.local/bin/woven-switch

# OSD — chain after media/volume/brightness actions
bindsym XF86AudioRaiseVolume  exec sh -c "wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+ && ~/.local/bin/woven-osd volume"
bindsym XF86AudioLowerVolume  exec sh -c "wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%- && ~/.local/bin/woven-osd volume"
bindsym XF86AudioMute         exec sh -c "wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle && ~/.local/bin/woven-osd volume"
bindsym XF86MonBrightnessUp   exec sh -c "brightnessctl set 10%+ && ~/.local/bin/woven-osd bright"
bindsym XF86MonBrightnessDown exec sh -c "brightnessctl set 10%- && ~/.local/bin/woven-osd bright"
bindsym XF86AudioPlay         exec sh -c "playerctl play-pause && ~/.local/bin/woven-osd media"

# Window rules
for_window [app_id="woven"]       border none
for_window [app_id="woven-popup"] floating enable, border none
```

---

## Systemd Service

Run `woven-session` as a systemd user service:

```bash
mkdir -p ~/.config/systemd/user
cp woven-session.service ~/.config/systemd/user/
systemctl --user enable woven-session
systemctl --user start woven-session
```

Or just add `exec ~/.local/bin/woven-session` to your Sway config's autostart section.

---

## Component Details

### woven-bar

Persistent top bar with workspace switcher, window title, system clock, battery indicator, audio volume, and network status.

Configuration: `~/.config/woven-shell/bar.toml`

### woven-launch

App launcher overlay. Search and launch applications.

Configuration: `~/.config/woven-shell/launch.toml`

### woven-lock

Lock screen with PAM authentication. Press Escape to cancel.

Configuration: `~/.config/woven-shell/lock.toml`

### woven-wall

Wallpaper daemon. Supports static image or gradient backgrounds.

Configuration: `~/.config/woven-shell/wall.toml`

### woven-cfg

Config manager GUI. Opens with `$mod+o`. Edit all woven-shell settings and Sway keybinds live, then save to regenerate configs and reload Sway.

The keybind source of truth is `~/.config/woven-shell/keybinds.toml` — don't edit the generated `~/.config/sway/woven-keybinds` directly.

### woven-osd

Persistent daemon. Start it once at login. The client mode (`woven-osd volume`, `woven-osd bright`, `woven-osd media`) sends commands to the running daemon over `/tmp/woven-osd.sock`.

### woven-screenshot

Screenshot utility with multiple modes:

- `woven-screenshot full` — Capture entire screen
- `woven-screenshot area` — Select area with slurp
- `woven-screenshot window` — Capture focused window

Screenshots are saved to `~/Pictures/Screenshots` with timestamps.

Requires: `slurp`, `grim`

### woven-session

Session daemon and IPC hub. Tracks battery state, media state, and provides DBus integration for system events. Listens on `/tmp/woven-session.sock` for status queries.

### woven-switch

Workspace switcher. Cycles forward through Sway workspaces. Bind to a key like `$mod+Tab` to quickly navigate workspaces.

---

## Platform

- CachyOS / Arch · Sway compositor
- Fonts: Inconsolata + any Nerd Font (for icons)

---

## Development

Clone and build:

```bash
git clone https://github.com/viewerofall/woven-shell
cd woven-shell
cargo build --release
```

Run a single component:

```bash
cargo run --release -p woven-bar
```

Build and install with `install.sh`:

```bash
./install.sh --all
```

Create a release package:

```bash
./install.sh --package
# Output: dist/woven-shell.tar.gz
```

---

## License

MIT. Do whatever.
