#!/bin/bash
# Install Zenbook Duo overlay into the current Omarchy user session.

set -euo pipefail

REPO="$(cd "$(dirname "$0")" && pwd)"
HYPR_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/hypr"
BIN_DIR="$HOME/.local/bin"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
# The on-screen keyboard binary calls zenbook-duo-osk from here.
ZENBOOK_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/zenbook"
PLUGIN_ID="io.github.scrambletools.zenbook-duo-keyboard"
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
  grep -Fq -- "$needle" "$file" && return 0
  printf '\n%s\n' "$block" >>"$file"
}

if [[ -x $REPO/bin/omarchy-hw-asus-zenbook-duo ]] && ! "$REPO/bin/omarchy-hw-asus-zenbook-duo"; then
  echo "Warning: this machine does not look like a Zenbook Duo UX8406." >&2
fi

mkdir -p "$HYPR_DIR" "$BIN_DIR" "$UNIT_DIR" "$ZENBOOK_DIR"

backup "$HYPR_DIR/duo.lua"
backup "$BIN_DIR/zenbook-duo-keyboard-watch"
backup "$BIN_DIR/zenbook-duo-fnkeys"
backup "$UNIT_DIR/zenbook-duo-keyboard-watch.service"
backup "$UNIT_DIR/zenbook-duo-fnkeys.service"

ln -sfn "$REPO/config/hypr/duo.lua" "$HYPR_DIR/duo.lua"

# omarchy-config owns monitors.lua when that overlay is installed.
if [[ -L $HYPR_DIR/monitors.lua ]]; then
  echo "Leaving $HYPR_DIR/monitors.lua in place"
else
  backup "$HYPR_DIR/monitors.lua"
  ln -sfn "$REPO/config/hypr/monitors.lua" "$HYPR_DIR/monitors.lua"
fi
ln -sfn "$REPO/bin/zenbook-duo-keyboard-watch" "$BIN_DIR/zenbook-duo-keyboard-watch"
ln -sfn "$REPO/bin/zenbook-duo-fnkeys" "$BIN_DIR/zenbook-duo-fnkeys"
chmod +x "$REPO/bin/zenbook-duo-keyboard-watch" "$REPO/bin/zenbook-duo-fnkeys" "$REPO/bin/omarchy-hw-asus-zenbook-duo"
ln -sfn "$REPO/systemd/zenbook-duo-keyboard-watch.service" "$UNIT_DIR/zenbook-duo-keyboard-watch.service"
ln -sfn "$REPO/systemd/zenbook-duo-fnkeys.service" "$UNIT_DIR/zenbook-duo-fnkeys.service"

ensure_line "$HYPR_DIR/input.lua" "$STAMP" "$STAMP
require(\"hypr.duo\").apply_devices()"

for helper in zenbook-duo-rotate-watch zenbook-duo-auto-brightness zenbook-duo-osk; do
  chmod +x "$REPO/bin/$helper"
  ln -sfn "$REPO/bin/$helper" "$ZENBOOK_DIR/$helper"
done
ln -sfn "$REPO/systemd/zenbook-duo-rotate-watch.service" "$UNIT_DIR/zenbook-duo-rotate-watch.service"
ln -sfn "$REPO/systemd/zenbook-duo-auto-brightness.service" "$UNIT_DIR/zenbook-duo-auto-brightness.service"

ensure_line "$HYPR_DIR/bindings.lua" "$STAMP" "$STAMP
o.bind(\"SUPER + CTRL + K\", \"On-screen keyboard\", \"$ZENBOOK_DIR/zenbook-duo-osk toggle\")
o.bind(\"SUPER + XF86MonBrightnessDown\", \"Auto-brightness: back to default curve\", \"$ZENBOOK_DIR/zenbook-duo-auto-brightness reset\")"

systemctl --user daemon-reload
systemctl --user enable --now zenbook-duo-keyboard-watch.service
systemctl --user enable --now zenbook-duo-fnkeys.service
if command -v monitor-sensor >/dev/null; then
  systemctl --user enable --now zenbook-duo-rotate-watch.service
  systemctl --user enable --now zenbook-duo-auto-brightness.service
else
  echo "Auto-rotation and auto-brightness need iio-sensor-proxy: sudo pacman -S iio-sensor-proxy, then re-run" >&2
fi

# Bar only on the screen with workspace 1; rebuilt after every omarchy update.
HOOK_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/hooks/post-update.d"
mkdir -p "$HOOK_DIR"
chmod +x "$REPO/bin/zenbook-duo-bar"
ln -sfn "$REPO/bin/zenbook-duo-bar" "$HOOK_DIR/zenbook-duo-bar"
if command -v omarchy >/dev/null && [[ ! -d ${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/plugins/${USER:-$(id -un)}.bar ]]; then
  "$REPO/bin/zenbook-duo-bar" refresh </dev/null || true
fi

if command -v cargo >/dev/null; then
  if (cd "$REPO/keyboard" && cargo build --release --locked); then
    # Close a running keyboard so the next open runs the new build.
    pgrep -f 'zenbook/zenbook-duo-keyboard$' >/dev/null && { "$ZENBOOK_DIR/zenbook-duo-osk" close || true; }
    install -m 0755 "$REPO/keyboard/target/release/zenbook-duo-keyboard" "$ZENBOOK_DIR/zenbook-duo-keyboard"
    mkdir -p "$HOME/.config/omarchy/plugins"
    ln -sfn "$REPO/omarchy-plugin" "$HOME/.config/omarchy/plugins/$PLUGIN_ID"
    if command -v omarchy >/dev/null; then
      omarchy restart shell >/dev/null 2>&1 || true
      omarchy plugin enable "$PLUGIN_ID" || true
    fi
  else
    echo "On-screen keyboard build failed; see the cargo output above" >&2
  fi
else
  echo "On-screen keyboard needs Rust: sudo pacman -S rust, then re-run" >&2
fi

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
echo "Fn row (USB and Bluetooth): media keys by default, hold Fn for F1-F12; F4 cycles keyboard backlight. The lamp turns off when the lid closes."
echo "Trackpad palm rejection: disable-while-typing + libinput TPK combo quirks."
echo "Screens rotate with the device; brightness follows the light sensor (Super+F5 resets it)."
echo "On-screen keyboard and touchpad on the bottom screen: Super+Ctrl+K or the bar button."
