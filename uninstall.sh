#!/bin/bash
# Remove the Zenbook Duo overlay from the current Omarchy user session.

set -euo pipefail

HYPR_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/hypr"
BIN_DIR="$HOME/.local/bin"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
STAMP="-- omarchy-zenbookduo"
TEMPLATE="${OMARCHY_PATH:-/usr/share/omarchy}/config/hypr/monitors.lua"

systemctl --user disable --now zenbook-duo-keyboard-watch.service 2>/dev/null || true
systemctl --user disable --now zenbook-duo-fnkeys.service 2>/dev/null || true
rm -f "$UNIT_DIR/zenbook-duo-keyboard-watch.service" "$UNIT_DIR/zenbook-duo-fnkeys.service"
rm -f "$BIN_DIR/zenbook-duo-keyboard-watch" "$BIN_DIR/zenbook-duo-fnkeys"
rm -f "$HYPR_DIR/duo.lua"

if [[ -L $HYPR_DIR/monitors.lua ]]; then
  case $(readlink -f "$HYPR_DIR/monitors.lua") in
    *omarchy-zenbookduo/config/hypr/monitors.lua)
      rm -f "$HYPR_DIR/monitors.lua"
      if [[ -f $TEMPLATE ]]; then
        cp "$TEMPLATE" "$HYPR_DIR/monitors.lua"
      fi
      ;;
  esac
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

remove_system() {
  rm -f /etc/libinput/omarchy-zenbookduo.quirks \
    /etc/udev/hwdb.d/61-omarchy-zenbookduo.hwdb \
    /etc/udev/rules.d/61-omarchy-zenbookduo.rules
  systemd-hwdb update
  udevadm control --reload-rules || true
  udevadm trigger --subsystem-match=input --action=change || true
}

if [[ $EUID -eq 0 ]]; then
  remove_system
elif sudo -n true 2>/dev/null; then
  sudo bash -c "$(declare -f remove_system); remove_system"
elif command -v pkexec >/dev/null; then
  pkexec /bin/bash -c 'rm -f /etc/libinput/omarchy-zenbookduo.quirks /etc/udev/hwdb.d/61-omarchy-zenbookduo.hwdb /etc/udev/rules.d/61-omarchy-zenbookduo.rules; systemd-hwdb update; udevadm control --reload-rules; udevadm trigger --subsystem-match=input --action=change'
fi

systemctl --user daemon-reload

if command -v hyprctl >/dev/null && [[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} ]]; then
  hyprctl reload >/dev/null || true
fi

echo "Removed Zenbook Duo overlay."
