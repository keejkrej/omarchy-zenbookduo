#!/bin/bash
# Regression tests for Duo dual-panel brightness sync.
# Mock sysfs by default (no screen flash). Set LIVE=1 to also drive the
# real omarchy-brightness-display path against this machine.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WATCH="$ROOT/bin/zenbook-duo-keyboard-watch"
fail=0

assert_eq() {
  local got=$1 want=$2 msg=$3
  if [[ $got == "$want" ]]; then
    printf 'ok  %s\n' "$msg"
  else
    printf 'FAIL %s: got %s want %s\n' "$msg" "$got" "$want"
    fail=1
  fi
}

# --- scale math ---
assert_eq "$("$WATCH" scale 200 400 400)" 200 "same-range 50%"
assert_eq "$("$WATCH" scale 400 400 400)" 400 "same-range 100%"
assert_eq "$("$WATCH" scale 0 400 400)" 1 "floor at 1, not 0"
assert_eq "$("$WATCH" scale 200 400 255)" 127 "scale onto 255 max"
assert_eq "$("$WATCH" scale 400 400 255)" 255 "clamp to dest max"

# --- mock sysfs copy ---
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/intel_backlight" "$tmp/card1-eDP-2-backlight"
printf '400\n' >"$tmp/intel_backlight/max_brightness"
printf '200\n' >"$tmp/intel_backlight/brightness"
printf '400\n' >"$tmp/card1-eDP-2-backlight/max_brightness"
printf '399\n' >"$tmp/card1-eDP-2-backlight/brightness"

BACKLIGHT_CLASS="$tmp" BACKLIGHT_LOCK="$tmp/lock" \
  "$WATCH" sync-backlight
assert_eq "$(cat "$tmp/card1-eDP-2-backlight/brightness")" 200 \
  "sync copies 50% from intel_backlight to eDP-2"

printf '40\n' >"$tmp/intel_backlight/brightness"
BACKLIGHT_CLASS="$tmp" BACKLIGHT_LOCK="$tmp/lock" \
  "$WATCH" sync-backlight
assert_eq "$(cat "$tmp/card1-eDP-2-backlight/brightness")" 40 \
  "sync follows a later intel_backlight change"

printf '40\n' >"$tmp/card1-eDP-2-backlight/brightness"
BACKLIGHT_CLASS="$tmp" BACKLIGHT_LOCK="$tmp/lock" \
  "$WATCH" sync-backlight
assert_eq "$(cat "$tmp/card1-eDP-2-backlight/brightness")" 40 \
  "sync is a no-op when already matched"

if [[ ${LIVE:-0} != 1 ]]; then
  if (( fail )); then
    echo "FAILED"
    exit 1
  fi
  echo "PASSED"
  exit 0
fi

# --- live path: Omarchy brightness keys only write intel_backlight ---
intel=/sys/class/backlight/intel_backlight
edp2=/sys/class/backlight/card1-eDP-2-backlight
[[ -e $intel/brightness && -e $edp2/brightness ]] || {
  echo "LIVE skip: backlight sysfs missing"
  exit 0
}

orig=$(<"$intel/brightness")
orig_bot=$(<"$edp2/brightness")
imax=$(<"$intel/max_brightness")
bmax=$(<"$edp2/max_brightness")
restore() {
  brightnessctl -d intel_backlight set "$orig" >/dev/null || true
  brightnessctl -d card1-eDP-2-backlight set "$orig_bot" >/dev/null || true
}
trap 'restore; rm -rf "$tmp"' EXIT
omarchy-brightness-display --no-osd 30%
cur=$(<"$intel/brightness")
want=$("$WATCH" scale "$cur" "$imax" "$bmax")
got=""
for _ in $(seq 1 30); do
  got=$(<"$edp2/brightness")
  [[ $got == "$want" ]] && break
  sleep 0.1
done
assert_eq "$got" "$want" "live eDP-2 follows omarchy-brightness-display 30%"
restore
trap 'rm -rf "$tmp"' EXIT

if (( fail )); then
  echo "FAILED"
  exit 1
fi
echo "PASSED"
