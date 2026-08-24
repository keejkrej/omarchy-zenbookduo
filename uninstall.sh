#!/bin/bash
# Remove the Zenbook Duo overlay from the current Omarchy user session.

set -euo pipefail

HYPR_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/hypr"
BIN_DIR="$HOME/.local/bin"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
STAMP="-- omarchy-zenbookduo"
TEMPLATE="${OMARCHY_PATH:-/usr/share/omarchy}/config/hypr/monitors.lua"

systemctl --user disable --now zenbook-duo-keyboard-watch.service 2>/dev/null || true
rm -f "$UNIT_DIR/zenbook-duo-keyboard-watch.service"
rm -f "$BIN_DIR/zenbook-duo-keyboard-watch"
rm -f "$HYPR_DIR/duo.lua"

if [[ -L $HYPR_DIR/monitors.lua ]]; then
  rm -f "$HYPR_DIR/monitors.lua"
  if [[ -f $TEMPLATE ]]; then
    cp "$TEMPLATE" "$HYPR_DIR/monitors.lua"
  fi
fi

if [[ -f $HYPR_DIR/input.lua ]]; then
  awk -v stamp="$STAMP" '
    $0 == stamp { skip=1; next }
    skip && /apply_devices/ { skip=0; next }
    { print }
  ' "$HYPR_DIR/input.lua" >"$HYPR_DIR/input.lua.tmp"
  mv "$HYPR_DIR/input.lua.tmp" "$HYPR_DIR/input.lua"
fi

if [[ -f $HYPR_DIR/autostart.lua ]]; then
  sed -i '/zenbook-duo-keyboard-watch/d' "$HYPR_DIR/autostart.lua"
fi

systemctl --user daemon-reload

if command -v hyprctl >/dev/null && [[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} ]]; then
  hyprctl reload >/dev/null || true
fi

echo "Removed Zenbook Duo overlay."
