#!/bin/bash
# Install Zenbook Duo overlay into the current Omarchy user session.

set -euo pipefail

REPO="$(cd "$(dirname "$0")" && pwd)"
HYPR_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/hypr"
BIN_DIR="$HOME/.local/bin"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
STAMP="-- omarchy-zenbookduo"

backup() {
  local path="$1"
  [[ -e $path || -L $path ]] || return 0
  [[ -L $path ]] && return 0
  cp -a "$path" "$path.bak.$(date +%s)"
}

ensure_line() {
  local file="$1"
  local needle="$2"
  local block="$3"
  mkdir -p "$(dirname "$file")"
  [[ -f $file ]] || printf '%s\n' "-- Extra autostart processes." >"$file"
  grep -Fq "$needle" "$file" && return 0
  printf '\n%s\n' "$block" >>"$file"
}

if [[ -x $REPO/bin/omarchy-hw-asus-zenbook-duo ]] && ! "$REPO/bin/omarchy-hw-asus-zenbook-duo"; then
  echo "Warning: this machine does not look like a Zenbook Duo UX8406." >&2
fi

mkdir -p "$HYPR_DIR" "$BIN_DIR" "$UNIT_DIR"

backup "$HYPR_DIR/duo.lua"
backup "$HYPR_DIR/monitors.lua"
backup "$BIN_DIR/zenbook-duo-keyboard-watch"
backup "$BIN_DIR/zenbook-duo-fnkeys"
backup "$UNIT_DIR/zenbook-duo-keyboard-watch.service"
backup "$UNIT_DIR/zenbook-duo-fnkeys.service"

ln -sfn "$REPO/config/hypr/duo.lua" "$HYPR_DIR/duo.lua"
ln -sfn "$REPO/config/hypr/monitors.lua" "$HYPR_DIR/monitors.lua"
ln -sfn "$REPO/bin/zenbook-duo-keyboard-watch" "$BIN_DIR/zenbook-duo-keyboard-watch"
ln -sfn "$REPO/bin/zenbook-duo-fnkeys" "$BIN_DIR/zenbook-duo-fnkeys"
chmod +x "$REPO/bin/zenbook-duo-keyboard-watch" "$REPO/bin/zenbook-duo-fnkeys" "$REPO/bin/omarchy-hw-asus-zenbook-duo"
ln -sfn "$REPO/systemd/zenbook-duo-keyboard-watch.service" "$UNIT_DIR/zenbook-duo-keyboard-watch.service"
ln -sfn "$REPO/systemd/zenbook-duo-fnkeys.service" "$UNIT_DIR/zenbook-duo-fnkeys.service"

ensure_line "$HYPR_DIR/input.lua" "$STAMP" "$STAMP
require(\"hypr.duo\").apply_devices()"

systemctl --user daemon-reload
systemctl --user enable --now zenbook-duo-keyboard-watch.service
systemctl --user enable --now zenbook-duo-fnkeys.service

install_system() {
  local sys="$REPO/bin/install-system.sh"
  chmod +x "$sys"
  if [[ $EUID -eq 0 ]]; then
    "$sys"
  elif sudo -n true 2>/dev/null; then
    sudo "$sys"
  elif command -v pkexec >/dev/null; then
    pkexec "$sys"
  else
    echo "Palm rejection needs root to install libinput quirks:" >&2
    echo "  sudo $sys" >&2
    return 1
  fi
}

install_system || true

if command -v hyprctl >/dev/null && [[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} ]]; then
  hyprctl reload >/dev/null
  hyprctl configerrors
fi

echo "Installed Zenbook Duo overlay from $REPO"
echo "Bottom display turns off while the keyboard is snapped on over USB."
echo "Fn row: media keys by default, hold Fn for F1-F12; F4 cycles keyboard backlight."
echo "Trackpad palm rejection: disable-while-typing + libinput TPK combo quirks."
