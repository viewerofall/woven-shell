#!/usr/bin/env bash
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="${HOME}/.local/bin"
CFG_DIR="${HOME}/.config/woven-shell"

COMPONENTS=(woven-bar woven-power woven-cc woven-launch woven-lock woven-wall woven-pick woven-cfg woven-osd woven-screenshot woven-session woven-switch)

# Config files per component (space-separated, relative to config/)
declare -A COMPONENT_CONFIGS=(
    [woven-bar]="bar.toml"
    [woven-launch]="launch.toml"
    [woven-lock]="lock.toml"
    [woven-wall]="wall.toml"
    [woven-cc]=""
    [woven-power]=""
    [woven-pick]=""
    [woven-cfg]=""
    [woven-osd]=""
    [woven-screenshot]=""
    [woven-session]=""
    [woven-switch]=""
)

# ── Helpers ───────────────────────────────────────────────────────────────────

bold()  { printf '\033[1m%s\033[0m\n' "$*"; }
info()  { printf '  \033[34m→\033[0m %s\n' "$*"; }
ok()    { printf '  \033[32m✓\033[0m %s\n' "$*"; }
skip()  { printf '  \033[33m–\033[0m %s\n' "$*"; }
die()   { printf '\033[31merror:\033[0m %s\n' "$*" >&2; exit 1; }

ask() {
    local prompt="$1"
    local reply
    read -r -p "  $prompt [y/N] " reply
    [[ "${reply,,}" == "y" ]]
}

check_deps() {
    command -v cargo &>/dev/null || die "cargo not found — install Rust from https://rustup.rs"
    mkdir -p "$BIN_DIR" "$CFG_DIR"
}

build_component() {
    local name="$1"
    info "Building $name..."
    cargo build --release -p "$name" --manifest-path "$REPO_DIR/Cargo.toml" 2>&1 \
        | grep -E '^(error|warning\[|Compiling|Finished)' || true
}

install_component() {
    local name="$1"
    local bin="$REPO_DIR/target/release/$name"
    [[ -f "$bin" ]] || die "binary not found after build: $bin"
    cp "$bin" "$BIN_DIR/$name"
    ok "Installed $BIN_DIR/$name"

    local configs="${COMPONENT_CONFIGS[$name]:-}"
    for cfg in $configs; do
        local src="$REPO_DIR/config/$cfg"
        local dst="$CFG_DIR/$cfg"
        if [[ -f "$src" ]]; then
            if [[ -f "$dst" ]]; then
                skip "Config $cfg already exists — skipping (use --force to overwrite)"
            else
                cp "$src" "$dst"
                ok "Installed $dst"
            fi
        fi
    done
}

install_themes() {
    if [[ -d "$REPO_DIR/config/themes" ]]; then
        mkdir -p "$CFG_DIR/themes"
        cp -r "$REPO_DIR/config/themes/"*.toml "$CFG_DIR/themes/" 2>/dev/null || true
        ok "Installed themes to $CFG_DIR/themes"
    fi
}

install_desktop() {
    local icon_src="$REPO_DIR/woven-shell-cfg.png"
    local icon_dst="$HOME/.local/share/icons/woven-shell-cfg.png"
    local desktop_dst="$HOME/.local/share/applications/woven-cfg.desktop"

    if [[ -f "$icon_src" ]]; then
        mkdir -p "$HOME/.local/share/icons"
        cp "$icon_src" "$icon_dst"
        ok "Installed $icon_dst"
    fi

    mkdir -p "$HOME/.local/share/applications"
    cat > "$desktop_dst" <<EOF
[Desktop Entry]
Name=Woven Shell Config
Comment=Configure Woven Shell components
Exec=$BIN_DIR/woven-cfg
Icon=$icon_dst
Type=Application
Categories=Settings;
Terminal=false
EOF
    ok "Installed $desktop_dst"
}

# ── Package mode ──────────────────────────────────────────────────────────────

make_package() {
    bold "Building release package..."
    check_deps
    cargo build --release --manifest-path "$REPO_DIR/Cargo.toml" 2>&1 \
        | grep -E '^(error|Compiling|Finished)' || true

    local pkg_dir="$REPO_DIR/dist/woven-shell"
    local tar_out="$REPO_DIR/dist/woven-shell.tar.gz"
    rm -rf "$pkg_dir"
    mkdir -p "$pkg_dir/bin" "$pkg_dir/config"

    for name in "${COMPONENTS[@]}"; do
        cp "$REPO_DIR/target/release/$name" "$pkg_dir/bin/"
    done

    cp -r "$REPO_DIR/config/." "$pkg_dir/config/"
    cp "$REPO_DIR/install.sh" "$pkg_dir/"
    cp "$REPO_DIR/README.md" "$pkg_dir/" 2>/dev/null || true
    cp "$REPO_DIR/woven-shell-cfg.png" "$pkg_dir/" 2>/dev/null || true

    tar -czf "$tar_out" -C "$REPO_DIR/dist" woven-shell
    rm -rf "$pkg_dir"
    ok "Package: $tar_out"
}

# ── Main ──────────────────────────────────────────────────────────────────────

usage() {
    echo "Usage: install.sh [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  (none)              Interactive — prompt y/n for each component"
    echo "  --install <name>    Install a single component"
    echo "  --all               Install everything without prompting"
    echo "  --package           Build a distributable tarball into dist/"
    echo "  --force             Overwrite existing config files"
    echo "  --help              Show this message"
}

INSTALL_ONE=""
INSTALL_ALL=false
PACKAGE=false
FORCE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --install) INSTALL_ONE="$2"; shift 2 ;;
        --all)     INSTALL_ALL=true; shift ;;
        --package) PACKAGE=true; shift ;;
        --force)   FORCE=true; shift ;;
        --help)    usage; exit 0 ;;
        *) die "Unknown option: $1" ;;
    esac
done

if $PACKAGE; then
    make_package
    exit 0
fi

check_deps
bold "woven-shell installer"
echo ""

if [[ -n "$INSTALL_ONE" ]]; then
    # Validate
    local_found=false
    for c in "${COMPONENTS[@]}"; do [[ "$c" == "$INSTALL_ONE" ]] && local_found=true; done
    $local_found || die "Unknown component '$INSTALL_ONE'. Valid: ${COMPONENTS[*]}"

    build_component "$INSTALL_ONE"
    install_component "$INSTALL_ONE"
    [[ "$INSTALL_ONE" == "woven-cfg" ]] && install_desktop

elif $INSTALL_ALL; then
    for name in "${COMPONENTS[@]}"; do
        build_component "$name"
        install_component "$name"
    done
    install_themes
    install_desktop
    echo ""
    bold "Done. Launching config manager..."
    "$BIN_DIR/woven-cfg" 2>/dev/null &

else
    TO_BUILD=()
    for name in "${COMPONENTS[@]}"; do
        if ask "Install $name?"; then
            TO_BUILD+=("$name")
        else
            skip "Skipping $name"
        fi
    done

    echo ""
    if [[ ${#TO_BUILD[@]} -eq 0 ]]; then
        echo "Nothing selected."
        exit 0
    fi

    bold "Building and installing..."
    echo ""
    for name in "${TO_BUILD[@]}"; do
        build_component "$name"
        install_component "$name"
    done
    install_themes
    for name in "${TO_BUILD[@]}"; do
        [[ "$name" == "woven-cfg" ]] && install_desktop && break
    done
fi

echo ""
bold "Done. Binaries are in $BIN_DIR"
