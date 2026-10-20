# Omarchy on ASUS Zenbook Duo

Device overlay for [Omarchy](https://omarchy.org/) on the ASUS Zenbook Duo UX8406 (2024 UX8406MA and 2025 UX8406CA).

Machine-independent Omarchy customizations live in
[omarchy-config](https://github.com/keejkrej/omarchy-config). The Razer Blade
overlay is [omarchy-razerblade](https://github.com/keejkrej/omarchy-razerblade).

Linux does not turn the bottom OLED off when the keyboard snaps onto it. This overlay does that in Hyprland: USB pogo-pin attach disables `eDP-2`, detach stacks it back under the top panel.

## What it does

- Turns **eDP-2 off** while the keyboard is snapped on over USB (`0b05:1bf2` on UX8406CA, `0b05:1b2c` on UX8406MA).
- On detach, re-enables eDP-2 under the top panel and wakes it (DPMS, workspace, backlight). Hyprland can otherwise list the second monitor while the OLED stays black.
- Syncs brightness from the top panel (`intel_backlight`) to the bottom OLED (`card1-eDP-2-backlight`). Omarchy's brightness keys only write the first backlight.
- Leaves both screens on when the keyboard is used over **Bluetooth** (not covering the glass).
- Maps each ELAN digitizer/stylus to the panel it sits on, and inhibits the bottom digitizer while the keyboard is covering it.
- Stacks the bottom panel under the top one (`auto-down`) in dual-screen mode. `apply_monitors` takes optional `top` and `bottom` positions so another config can place the pair.
- Trackpad palm rejection: treat the Duo keyboard+touchpad as a laptop combo (`AttrTPKComboLayout=below`), enable disable-while-typing, and ignore the extra relative-mouse node. The touchpad does not report contact size, so resting palms *without* typing is still limited.
- Resets the bottom panel's touch controller two seconds after the keyboard comes off. The capacitive panel takes its no-touch baseline while the magnetic keyboard lies on it and then needs a hard press; rebinding i2c-hid makes it recalibrate on clear glass.
- Fn row on the USB **and Bluetooth** keyboard: media keys by default (mute / volume / brightness), hold Fn for F1–F12. Fn+Esc toggles that so F1–F12 become the default. F4 cycles keyboard backlight. The lamp turns off when the lid closes or the machine suspends, and comes back at the same level when the lid is open again. Mic/F9 is hold-to-talk dictation. Emoji key opens the Omarchy emoji picker. Dictation toggle/PTT otherwise stay Omarchy defaults (Super+Ctrl+X / F9). A udev `uaccess` rule lets the user-session daemon open the vendor hidraw without membership in group `input`.

### Rotation, sharing mode, auto-brightness

- **Auto-rotation** from the accelerometer (`iio-sensor-proxy`): on its side the panels go side by side with workspace 1 on the left one, upside down they swap. Touch and stylus input turn with their panel. Touch `~/.local/state/zenbook/rotation-lock` to hold the layout.
- **Sharing mode**: opened flat (over 170°) and lying on a table, the top panel turns around for the person opposite. On the UX8406MA the sensor hub's Intel hinge sensor reads 0 on every channel, but the hub also exposes an accelerometer in each half as ASUS custom sensors (`HID-SENSOR-2000e1.*`, custom usage `0x73`), reachable only through hid-sensor-custom's root-only `/dev` nodes. `zenbook-duo-hinge.service` (root) computes the opening angle from the two gravity vectors and writes it to `/run/zenbook-duo/hinge`. Where those sensors are missing it exits and the kernel's hinge sensor is used.
- **Auto-brightness** from the light sensor. Manual changes are kept as an offset to the curve; Super+F5 resets it, `~/.local/state/zenbook/auto-brightness-off` pauses it.

### On-screen keyboard and touchpad

Super+Ctrl+K, or the bar button while the keyboard is off, shows a copy of the detachable keyboard and its touchpad on the bottom panel (Rust, iced on a layer-shell surface, virtual keyboard and pointer; no root). It closes when the keyboard snaps back on.

- Colours from the current Omarchy theme, following theme switches, inside Hyprland's window gap, border and rounding.
- With a `fi` or `se` kb_layout (or `ZENBOOK_DUO_OSK_LAYOUT=fi`) it shows and types the Finnish/Swedish ISO layout, with AltGr and third-level legends; otherwise US.
- Rendered with wgpu. tiny-skia, kept as the fallback, redrew the whole panel on the CPU for every key: 283 ms median from touch to frame on a UX8406MA, against 4 ms now.

### Bar on one screen

`zenbook-duo-bar` clones Omarchy's bar (`omarchy plugin clone omarchy.bar`) and drops its panel on whichever built-in screen does not hold workspace 1. It is installed as an Omarchy post-update hook, so the clone is rebuilt from each updated bar; if the bar's code no longer matches, it switches back to the standard bar and says so. `zenbook-duo-bar remove` undoes it.

### SDDM greeter

The greeter puts a login view on every screen; with the keyboard snapped on, keyboard focus could sit in the hidden bottom view. A greeter config that loads Omarchy's and disables eDP-2 is selected by `/etc/sddm.conf.d/zz-zenbook-duo.conf`.

### Face unlock (optional)

`sudo bin/zenbook-duo-faceid setup`, then `sudo bin/zenbook-duo-faceid enroll $USER`, with `howdy-next` from the AUR. It finds the IR camera (the greyscale-only capture node; its emitter strobes without `linux-enable-ir-emitter`) and adds Howdy to sudo, polkit, SDDM and the Omarchy lock screen, backing up each PAM file. The lock screen only starts its automatic stack (`omarchy-lock-fingerprint`) when `fprintd-list` mentions a finger, and retries 250 ms after a failure, so a `/usr/local/bin/fprintd-list` wrapper reports the Howdy stack and a gate skips the camera while the panels are blanked. `sudo bin/zenbook-duo-faceid remove` restores every PAM file. Not run by `install.sh`.

Rotation, sharing mode, auto-brightness, the on-screen keyboard and the bar filter are adapted from [scrambletoolsllc/zenbook-duo-omarchy](https://github.com/scrambletoolsllc/zenbook-duo-omarchy) (UX8407AA, MIT); the face unlock idea is from [laithm/zenbook-duo-hyprland](https://github.com/laithm/zenbook-duo-hyprland).

## Install

On Omarchy:

```bash
git clone https://github.com/keejkrej/omarchy-zenbookduo.git ~/workspace/omarchy-zenbookduo
~/workspace/omarchy-zenbookduo/install.sh
```

`install.sh` symlinks the Hyprland Duo helpers and watchers from this repo, then enables the user systemd services. Auto-rotation and auto-brightness need `iio-sensor-proxy`; the on-screen keyboard is built when `cargo` is installed. If [omarchy-config](https://github.com/keejkrej/omarchy-config) already owns `~/.config/hypr/monitors.lua`, that file is left in place (`hypr.duo.apply_monitors` runs from there). Otherwise this overlay installs its own `monitors.lua` (scale `1.6`).

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
bin/zenbook-duo-keyboard-watch    # USB snap watcher + dual-panel brightness sync
bin/zenbook-duo-fnkeys            # USB/Bluetooth Fn-row + keyboard backlight
bin/install-system.sh             # libinput quirks, udev hwdb/rules, hinge service, SDDM greeter (root)
bin/zenbook-duo-rotate-watch      # accelerometer + hinge -> rotation / sharing layout
bin/zenbook-duo-hinge             # hinge angle from the two halves' accelerometers (root)
bin/zenbook-duo-auto-brightness   # ambient light -> top backlight
bin/zenbook-duo-osk               # open/close the on-screen keyboard, park workspaces
bin/zenbook-duo-touch-reset       # recalibrate the bottom touch panel after undock (root)
bin/zenbook-duo-bar               # bar only on the workspace-1 screen (post-update hook)
bin/zenbook-duo-faceid            # optional Howdy face unlock (root)
keyboard/                         # on-screen keyboard (Rust)
omarchy-plugin/                   # on-screen keyboard bar button
config/sddm/                      # top-panel-only greeter
config/hypr/duo.lua               # snap detection, monitors, digitizers, trackpad
config/hypr/monitors.lua          # standalone display layout (skipped if omarchy-config owns monitors.lua)
config/libinput/omarchy-zenbookduo.quirks
config/udev/61-omarchy-zenbookduo.hwdb
config/udev/61-omarchy-zenbookduo.rules   # hidraw TAG+=uaccess for the Fn-row daemon
systemd/zenbook-duo-keyboard-watch.service
systemd/zenbook-duo-fnkeys.service
systemd/zenbook-duo-rotate-watch.service
systemd/zenbook-duo-auto-brightness.service
systemd/zenbook-duo-hinge.service          # system service
```
