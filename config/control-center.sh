#!/usr/bin/env bash
# woven-shell control center
# Requires: rofi, brightnessctl, wpctl, swaylock, systemctl, swaymsg

OPTIONS=(
    "  Lock"
    "  Shutdown"
    "  Reboot"
    "  Suspend"
    "  Log Out"
    "──────────────────"
    "󰃠  Brightness +10%"
    "󰃞  Brightness -10%"
    "──────────────────"
    "󰕾  Volume +10%"
    "󰕿  Volume -10%"
    "󰝟  Mute Toggle"
)

CHOICE=$(printf '%s\n' "${OPTIONS[@]}" | grep -v '^──' | \
    rofi -dmenu -i -p "  Control Center" \
         -theme-str 'window { width: 280px; } listview { lines: 10; }')

case "$CHOICE" in
    *Lock)             swaylock -f ;;
    *Shutdown)         systemctl poweroff ;;
    *Reboot)           systemctl reboot ;;
    *Suspend)          systemctl suspend ;;
    *"Log Out")        swaymsg exit ;;
    *"+10%"*right*|*"Brightness +10%") brightnessctl set 10%+ ;;
    *"-10%"*right*|*"Brightness -10%") brightnessctl set 10%- ;;
    *"Volume +10%")    wpctl set-volume @DEFAULT_AUDIO_SINK@ 10%+ ;;
    *"Volume -10%")    wpctl set-volume @DEFAULT_AUDIO_SINK@ 10%- ;;
    *"Mute Toggle")    wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle ;;
esac
