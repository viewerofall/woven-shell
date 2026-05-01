#!/bin/sh
# woven-shell installer
# Usage: curl -fsSL https://raw.githubusercontent.com/viewerofall/woven-shell/main/get.sh | sh
#
# Install a single component:
# curl -fsSL https://raw.githubusercontent.com/viewerofall/woven-shell/main/get.sh | sh -s -- --install woven-osd

set -e

REPO="viewerofall/woven-shell"
TARBALL="woven-shell.tar.gz"
TMP=$(mktemp -d)
BINDIR="$HOME/.local/bin"
CFGDIR="$HOME/.config/woven-shell"

COMPONENTS="woven-bar woven-power woven-cc woven-launch woven-lock woven-wall woven-pick woven-cfg woven-osd woven-screenshot woven-session woven-switch"

cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT

# ── Arg parse ─────────────────────────────────────────────────────────────────

INSTALL_ONE=""
INSTALL_ALL=false

while [ $# -gt 0 ]; do
    case "$1" in
        --install) INSTALL_ONE="$2"; shift 2 ;;
        --all)     INSTALL_ALL=true; shift ;;
        --help)
            echo "Usage: get.sh [--install <component>] [--all]"
            echo "Components: $COMPONENTS"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# ── Download ──────────────────────────────────────────────────────────────────

echo "==> Downloading woven-shell..."
curl -fsSL "https://github.com/$REPO/releases/latest/download/$TARBALL" \
    -o "$TMP/$TARBALL"

echo "==> Extracting..."
tar -xzf "$TMP/$TARBALL" -C "$TMP"

SRC=$(find "$TMP" -maxdepth 1 -mindepth 1 -type d | head -1)
[ -z "$SRC" ] && SRC="$TMP"

mkdir -p "$BINDIR" "$CFGDIR"

# ── Install logic ─────────────────────────────────────────────────────────────

install_one() {
    name="$1"
    bin="$SRC/bin/$name"

    if [ ! -f "$bin" ]; then
        echo "  ! binary not found in package: $name"
        return 1
    fi

    cp "$bin" "$BINDIR/$name"
    chmod +x "$BINDIR/$name"
    echo "  ✓ $BINDIR/$name"

    # Copy matching config if present and not already there
    for cfg in "$SRC/config/"*.toml; do
        [ -f "$cfg" ] || continue
        base=$(basename "$cfg")
        dst="$CFGDIR/$base"
        if [ ! -f "$dst" ]; then
            cp "$cfg" "$dst"
            echo "  ✓ $CFGDIR/$base"
        else
            echo "  – $CFGDIR/$base already exists, skipping"
        fi
    done
}

ask() {
    printf "  Install %s? [y/N] " "$1"
    read -r reply
    [ "${reply}" = "y" ] || [ "${reply}" = "Y" ]
}

install_themes() {
    if [ -d "$SRC/config/themes" ]; then
        mkdir -p "$CFGDIR/themes"
        cp "$SRC/config/themes/"*.toml "$CFGDIR/themes/" 2>/dev/null || true
        echo "  ✓ Themes installed to $CFGDIR/themes"
    fi
}

install_desktop() {
    icon_src="$SRC/woven-shell-cfg.png"
    icon_dst="$HOME/.local/share/icons/woven-shell-cfg.png"
    desktop_dst="$HOME/.local/share/applications/woven-cfg.desktop"

    if [ -f "$icon_src" ]; then
        mkdir -p "$HOME/.local/share/icons"
        cp "$icon_src" "$icon_dst"
        echo "  ✓ $icon_dst"
    fi

    mkdir -p "$HOME/.local/share/applications"
    cat > "$desktop_dst" <<EOF
[Desktop Entry]
Name=Woven Shell Config
Comment=Configure Woven Shell components
Exec=$BINDIR/woven-cfg
Icon=$HOME/.local/share/icons/woven-shell-cfg.png
Type=Application
Categories=Settings;
Terminal=false
EOF
    echo "  ✓ $desktop_dst"
}

echo ""

if [ -n "$INSTALL_ONE" ]; then
    echo "==> Installing $INSTALL_ONE..."
    install_one "$INSTALL_ONE"
    [ "$INSTALL_ONE" = "woven-cfg" ] && install_desktop

elif $INSTALL_ALL; then
    echo "==> Installing all components..."
    for name in $COMPONENTS; do
        install_one "$name"
    done
    install_themes
    install_desktop
    echo ""
    echo "==> Launching config manager..."
    "$BINDIR/woven-cfg" 2>/dev/null &

else
    echo "==> Select components to install:"
    echo ""
    TO_INSTALL=""
    for name in $COMPONENTS; do
        if ask "$name"; then
            TO_INSTALL="$TO_INSTALL $name"
        else
            echo "  – skipping $name"
        fi
    done

    echo ""
    if [ -z "$TO_INSTALL" ]; then
        echo "Nothing selected. Exiting."
        exit 0
    fi

    echo "==> Installing..."
    for name in $TO_INSTALL; do
        install_one "$name"
    done
    install_themes
    for name in $TO_INSTALL; do
        [ "$name" = "woven-cfg" ] && install_desktop && break
    done
fi

echo ""
echo "==> Done. Binaries installed to $BINDIR"
echo ""
echo "    Make sure $BINDIR is in your PATH."
echo ""
echo "    Add to ~/.config/sway/config:"
echo "      exec_always --no-startup-id $BINDIR/woven-bar"
echo "      exec_always --no-startup-id $BINDIR/woven-wall"
echo "      exec $BINDIR/woven-osd"
echo "      bindsym \$mod+d      exec $BINDIR/woven-launch"
echo "      bindsym \$mod+Shift+p exec $BINDIR/woven-power"
echo "      bindsym \$mod+Escape  exec $BINDIR/woven-lock"
echo "      bindsym \$mod+o       exec $BINDIR/woven-cfg"
echo ""
