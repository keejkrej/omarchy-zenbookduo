# Omarchy on ASUS Zenbook Duo

Device overlay for [Omarchy](https://omarchy.org/) on the ASUS Zenbook Duo UX8406 (2024 UX8406MA and 2025 UX8406CA).

Linux does not turn the bottom OLED off when the keyboard snaps onto it. This overlay does that in Hyprland: USB pogo-pin attach disables `eDP-2`, detach stacks it back under the top panel.

## What it does

- Turns **eDP-2 off** while the keyboard is snapped on over USB (`0b05:1bf2` on UX8406CA, `0b05:1b2c` on UX8406MA).
- Leaves both screens on when the keyboard is used over **Bluetooth** (not covering the glass).
- Maps each ELAN digitizer/stylus to the panel it sits on, and inhibits the bottom digitizer while the keyboard is covering it.
- Stacks the bottom panel under the top one (`auto-down`) in dual-screen mode.
- Trackpad palm rejection: treat the Duo keyboard+touchpad as a laptop combo (`AttrTPKComboLayout=below`), enable disable-while-typing, and ignore the extra relative-mouse node. The touchpad does not report contact size, so resting palms *without* typing is still limited.
- Fn row on the USB keyboard: media keys by default (mute / volume / brightness / mic mute), hold Fn for F1–F12. F4 cycles keyboard backlight. Emoji key opens the Omarchy emoji picker.

## Install

On Omarchy:

```bash
git clone https://github.com/keejkrej/omarchy-zenbookduo.git ~/workspace/omarchy-zenbookduo
~/workspace/omarchy-zenbookduo/install.sh
```

`install.sh` symlinks the Hyprland files and watcher from this repo, then enables a user systemd service. Scale defaults to `1.6` in `config/hypr/monitors.lua`; edit that file if you want a different scale.

Remove with `./uninstall.sh`.

## Sharing this with Omarchy

This is **not** an Omarchy shell plugin (`omarchy plugin add` is for bar/QML widgets). It is a hardware overlay, the same kind of thing Omarchy already ships under `install/hardware/` and `bin/omarchy-hw-*`.

Places to share it:

1. **Discord** — [omarchy.org/discord](https://omarchy.org/discord) (invite `https://discord.gg/tXFUdasqhY`). Best first stop: other Duo owners, feedback, and whether official support is wanted.
2. **GitHub Discussions (suggestions)** — [basecamp/omarchy discussions](https://github.com/basecamp/omarchy/discussions/categories/suggestions). Use this to propose first-party UX8406 support, with a link to this repo.
3. **Upstream PR** — clone [basecamp/omarchy](https://github.com/basecamp/omarchy) and follow its `AGENTS.md`. The pieces that would land there are:
   - `bin/omarchy-hw-asus-zenbook-duo`
   - a watcher next to `omarchy-hyprland-monitor-watch`
   - Hyprland rules loaded only when that hw helper matches
   - an `install/hardware/asus/` script

Issues on `basecamp/omarchy` are for validated bugs in Omarchy itself, not for dropping a hardware overlay.

## Layout

```
bin/omarchy-hw-asus-zenbook-duo   # UX8406 detector
bin/zenbook-duo-keyboard-watch    # USB snap watcher (reloads Hyprland)
bin/zenbook-duo-fnkeys            # USB Fn-row + keyboard backlight
bin/install-system.sh             # libinput quirks + udev hwdb/rules (root)
config/hypr/duo.lua               # snap detection, monitors, digitizers, trackpad
config/hypr/monitors.lua          # Duo display layout
config/libinput/omarchy-zenbookduo.quirks
config/udev/61-omarchy-zenbookduo.hwdb
config/udev/61-omarchy-zenbookduo.rules
systemd/zenbook-duo-keyboard-watch.service
systemd/zenbook-duo-fnkeys.service
```
