# woven-shell

A complete Wayland shell for Sway. Custom bar, launcher, lock screen, wallpaper daemon, power menu, control center, config manager, and OSD — all in Rust.

Theme: `#0a0010` bg · `#c792ea` accent · `#00e5c8` teal

---

## Quick install

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

---

## Requirements

- **Sway** compositor — wlr-layer-shell, Sway IPC
- Rust stable toolchain
- `pam` development headers (for woven-lock)
- Runtime: `wpctl`, `brightnessctl`, `playerctl`, `swaync`

---

## Build from source

```bash
git clone https://github.com/viewerofall/woven-shell
cd woven-shell
cargo build --release
cp target/release/woven-{bar,power,cc,launch,lock,wall,pick,cfg,osd} ~/.local/bin/
mkdir -p ~/.config/woven-shell && cp config/*.toml ~/.config/woven-shell/
```

---

## Sway config

Add to your `~/.config/sway/config`:

```
# Autostart
exec_always --no-startup-id ~/.local/bin/woven-wall
exec_always --no-startup-id ~/.local/bin/woven-bar
exec_always --no-startup-id /usr/bin/swaync
exec ~/.local/bin/woven-osd

# Keybinds
bindsym $mod+Return      exec kitty
bindsym $mod+d           exec ~/.local/bin/woven-launch
bindsym $mod+q           kill
bindsym $mod+Shift+p     exec ~/.local/bin/woven-power
bindsym $mod+o           exec ~/.local/bin/woven-cfg
bindsym $mod+Escape      exec ~/.local/bin/woven-lock

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

## woven-cfg

`$mod+o` opens the config manager. It edits `~/.config/woven-shell/*.toml` and regenerates `~/.config/sway/woven-keybinds` on save, then reloads Sway automatically.

The keybind source of truth is `~/.config/woven-shell/keybinds.toml` — don't edit the generated `~/.config/sway/woven-keybinds` directly.

---

## woven-osd

Persistent daemon. Start it once at login (`exec ~/.local/bin/woven-osd` in sway config). The client mode (`woven-osd volume`, `woven-osd bright`, `woven-osd media`) sends a command to the running daemon over `/tmp/woven-osd.sock`.

---

## Platform

- CachyOS / Arch · Sway compositor
- Fonts: Inconsolata + any Nerd Font (for icons in woven-osd)

---

## License

MIT. Do whatever.
