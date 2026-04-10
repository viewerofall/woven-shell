# woven-shell

Lil complete shell for woven with custom shit since **I** felt like it. 

---

## What's in here

| Crate | What it does |
|---|---|
| `woven-bar` | Persistent top bar — workspaces, window title, clock, tray, battery, audio, network |
| `woven-power` | Power menu overlay — shutdown, reboot, suspend, logout |
| `woven-cc` | Control center panel — quick toggles, sliders |

**Planned:**
- `woven-wall` — animated wallpaper daemon (gradient / static image)
- `woven-lock` — lock screen with PAM auth
- `woven-launch` — app launcher (rofi/wofi replacement)
- `woven-widgets` — desktop widgets (clock, calendar, sysmon)

---

## Requirements

- Sway compositor (woven-bar **relies** on it for now you cant use it without sway)
- Rust toolchain (stable)
- `pam` development headers (for woven-lock, when it lands)

---

## Install

```bash
git clone https://github.com/viewerofall/woven-shell
cd woven-shell
cargo build --release
```

Then move the binaries wherever you want them:

```bash
cp target/release/woven-bar ~/.local/bin/
cp target/release/woven-power ~/.local/bin/
cp target/release/woven-cc ~/.local/bin/
```

Config goes in `~/.config/woven-shell/` — copy the example:

```bash
mkdir -p ~/.config/woven-shell
cp config/ ~/.config/woven-shell/ -r
```


---

## Platform

- Arch, on sway
- Fonts: Inconsolata + Syne
- Theme: `#0a0010` / `#c792ea` / `#00e5c8`

---

## License

Do whatever. MIT.
