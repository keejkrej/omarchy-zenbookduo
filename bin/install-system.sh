#!/bin/bash
# Root half of the overlay: libinput quirks, udev hwdb, hidraw seat access.

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

systemd-hwdb update
udevadm control --reload-rules
udevadm trigger --subsystem-match=input --action=add
udevadm trigger --subsystem-match=hidraw --action=add
udevadm trigger --subsystem-match=hidraw --action=change

# Root daemon that turns the two halves' accelerometers into a hinge angle.
# A copy, not a link into the user's checkout, since it runs as root.
install -D -m 755 "$REPO/bin/zenbook-duo-hinge" /usr/local/lib/omarchy-zenbookduo/zenbook-duo-hinge
install -m 755 "$REPO/bin/zenbook-duo-touch-reset" /usr/local/lib/omarchy-zenbookduo/zenbook-duo-touch-reset
install -m 644 "$REPO/systemd/zenbook-duo-hinge.service" /etc/systemd/system/zenbook-duo-hinge.service
systemctl daemon-reload
systemctl enable zenbook-duo-hinge.service
systemctl restart zenbook-duo-hinge.service

echo "Installed Duo libinput quirks, touchpad hwdb, hidraw udev rules, and hinge service."
