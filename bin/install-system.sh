#!/bin/bash
# Root half of the overlay: libinput quirks, udev hwdb, hidraw/uinput access.

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
  echo "install-system.sh must run as root" >&2
  exit 1
fi

REPO="$(cd "$(dirname "$0")/.." && pwd)"

install -d /etc/libinput /etc/udev/hwdb.d /etc/udev/rules.d
install -m 644 "$REPO/config/libinput/omarchy-zenbookduo.quirks" /etc/libinput/omarchy-zenbookduo.quirks
install -m 644 "$REPO/config/udev/61-omarchy-zenbookduo.hwdb" /etc/udev/hwdb.d/61-omarchy-zenbookduo.hwdb
install -m 644 "$REPO/config/udev/61-omarchy-zenbookduo.rules" /etc/udev/rules.d/61-omarchy-zenbookduo.rules

modprobe uinput 2>/dev/null || true
systemd-hwdb update
udevadm control --reload-rules
udevadm trigger --subsystem-match=input --action=add
udevadm trigger --subsystem-match=hidraw --action=add
udevadm trigger --subsystem-match=hidraw --action=change
udevadm trigger --subsystem-match=misc --action=add

echo "Installed Duo libinput quirks, touchpad hwdb, and hidraw/uinput udev rules."
