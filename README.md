# Omarchy on ASUS Zenbook Duo

Device overlay for [Omarchy](https://omarchy.org/) on the ASUS Zenbook Duo UX8406 (2024 UX8406MA and 2025 UX8406CA).

Machine-independent Omarchy customizations live in
[omarchy-config](https://github.com/keejkrej/omarchy-config). The Razer Blade
overlay is [omarchy-razerblade](https://github.com/keejkrej/omarchy-razerblade).

Linux does not turn the bottom OLED off when the keyboard snaps onto it. This overlay does that in Hyprland: USB pogo-pin attach disables `eDP-2`, detach stacks it back under the top panel.

## What it does

- Turns **eDP-2 off** while the keyboard is snapped on over USB (`0b05:1bf2` on UX8406CA, `0b05:1b2c` on UX8406MA).
- On detach, re-enables eDP-2 under the top panel and wakes it (DPMS, workspace, backlight). Hyprland can otherwise list the second monitor while the OLED stays black.
- Leaves both screens on when the keyboard is used over **Bluetooth** (not covering the glass).
- Maps each ELAN digitizer/stylus to the panel it sits on, and inhibits the bottom digitizer while the keyboard is covering it.
- Stacks the bottom panel under the top one (`auto-down`) in dual-screen mode.
- Trackpad palm rejection: treat the Duo keyboard+touchpad as a laptop combo (`AttrTPKComboLayout=below`), enable disable-while-typing, and ignore the extra relative-mouse node. The touchpad does not report contact size, so resting palms *without* typing is still limited.
- Fn row on the USB keyboard: media keys by default (mute / volume / brightness), hold Fn for F1–F12. Fn+Esc toggles that so F1–F12 become the default. F4 cycles keyboard backlight. Mic/F9 is hold-to-talk dictation; Ctrl+Super taps to start or stop (Duo USB keyboard only). Emoji key opens the Omarchy emoji picker.

## Install

On Omarchy:

```bash
git clone https://github.com/keejkrej/omarchy-zenbookduo.git ~/workspace/omarchy-zenbookduo
~/workspace/omarchy-zenbookduo/install.sh
```

`install.sh` symlinks the Hyprland Duo helpers and watcher from this repo, then enables a user systemd service. If [omarchy-config](https://github.com/keejkrej/omarchy-config) already owns `~/.config/hypr/monitors.lua`, that file is left in place (`hypr.duo.apply_monitors` runs from there). Otherwise this overlay installs its own `monitors.lua` (scale `1.6`).

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
config/hypr/monitors.lua          # standalone display layout (skipped if omarchy-config owns monitors.lua)
config/libinput/omarchy-zenbookduo.quirks
config/udev/61-omarchy-zenbookduo.hwdb
config/udev/61-omarchy-zenbookduo.rules
systemd/zenbook-duo-keyboard-watch.service
systemd/zenbook-duo-fnkeys.service
```
