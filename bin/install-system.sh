#!/bin/bash
# Root half of the overlay: libinput quirks + udev hwdb for palm rejection.

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
  echo "install-system.sh must run as root" >&2
  exit 1
fi

REPO="$(cd "$(dirname "$0")/.." && pwd)"

install -d /etc/libinput /etc/udev/hwdb.d
install -m 644 "$REPO/config/libinput/omarchy-zenbookduo.quirks" /etc/libinput/omarchy-zenbookduo.quirks
install -m 644 "$REPO/config/udev/61-omarchy-zenbookduo.hwdb" /etc/udev/hwdb.d/61-omarchy-zenbookduo.hwdb

systemd-hwdb update
udevadm trigger --subsystem-match=input --action=add

echo "Installed Duo libinput quirks and touchpad hwdb."
