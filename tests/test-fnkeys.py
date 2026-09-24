#!/usr/bin/env python3
"""Regression tests for Duo Fn-row vendor reports and udev seat access."""

from __future__ import annotations

from importlib.machinery import SourceFileLoader
from importlib.util import module_from_spec, spec_from_loader
from pathlib import Path
from unittest.mock import MagicMock, patch

ROOT = Path(__file__).resolve().parents[1]
RULES = ROOT / "config/udev/61-omarchy-zenbookduo.rules"
loader = SourceFileLoader("fnkeys", str(ROOT / "bin/zenbook-duo-fnkeys"))
spec = spec_from_loader("fnkeys", loader)
assert spec is not None
fnkeys = module_from_spec(spec)
loader.exec_module(fnkeys)

fail = 0

# Captured from UX8406CA USB vendor hidraw (usage page 0xFF31 at the end).
HIDRAW5_DESC = bytes.fromhex(
    "0600ff0901a10185420906150026ff0075089503b10285430906150026ff007508"
    "9503b1020600ff85410905150026ff007508960001b102c00631ff0976a101855a"
    "0976150026ff00750895058102855a09767508950fb102c0"
)
# Consumer-control interface on the same keyboard — must not match.
HIDRAW1_DESC = bytes.fromhex("050c0901a101850319002aff02150026ff02751895018100c0")


def check(cond: bool, msg: str) -> None:
    global fail
    if cond:
        print(f"ok  {msg}")
    else:
        print(f"FAIL {msg}")
        fail = 1


def test_udev_rules() -> None:
    text = RULES.read_text()
    check("TAG+=\"uaccess\"" in text, "udev grants seat uaccess on Duo hidraw")
    check("GROUP=\"input\"" not in text, "udev does not depend on group input")
    check("KERNEL==\"uinput\"" not in text, "udev does not expose uinput")
    for product in ("1bf2", "1b2c", "1BF3", "1B2D"):
        check(product in text, f"udev matches product {product}")


def test_vendor_page() -> None:
    check(fnkeys.USAGE_PAGE_ASUS in HIDRAW5_DESC, "vendor hidraw descriptor has 0xFF31")
    check(fnkeys.USAGE_PAGE_ASUS not in HIDRAW1_DESC, "consumer hidraw descriptor is not vendor")


def test_lid_and_sleep_blank_backlight() -> None:
    """Closing the lid or suspending turns the lamp off without forgetting the level."""
    import fcntl
    import os
    import tempfile
    from unittest.mock import patch

    with tempfile.TemporaryDirectory() as tmp:
        state = Path(tmp)
        backlight_path = state / "kbd-backlight"
        env = {
            "STATE_DIR": state,
            "STATE_PATH": backlight_path,
            "FNLOCK_PATH": state / "fn-lock",
        }
        with patch.multiple(fnkeys, **env), patch.object(fnkeys, "hid_set_feature") as feat, patch.object(
            fnkeys.os, "open", return_value=11
        ), patch.object(fnkeys.os, "close"), patch.object(fnkeys, "osd"):
            fnkeys.save_backlight(2)
            kbd = fnkeys.Keyboard(Path("/dev/hidraw-test"))
            check(kbd.backlight == 2, "saved backlight level is restored on the open lid")
            feat.reset_mock()

            kbd.set_power_state(lid_closed=True, sleeping=False)
            check(feat.call_args.args[1] == fnkeys.BACKLIGHT[0], "lid close writes backlight 0")
            check(kbd.backlight == 2, "lid close keeps the saved level in memory")
            check(fnkeys.load_backlight() == 2, "lid close does not persist 0")

            feat.reset_mock()
            kbd.set_power_state(lid_closed=True, sleeping=False)
            check(not feat.called, "a second lid-close event does not rewrite the lamp")

            kbd.cycle_backlight()
            check(kbd.backlight == 3, "F4 while the lid is closed still advances the saved level")
            check(fnkeys.load_backlight() == 3, "F4 while the lid is closed persists the new level")
            check(
                all(call.args[1] == fnkeys.BACKLIGHT[0] for call in feat.call_args_list),
                "F4 while the lid is closed does not turn the lamp on",
            )

            feat.reset_mock()
            kbd.set_power_state(lid_closed=False, sleeping=True)
            check(not feat.called, "suspend while the lamp is already off does not rewrite it")

            kbd.set_power_state(lid_closed=False, sleeping=False)
            check(feat.call_args.args[1] == fnkeys.BACKLIGHT[3], "lid open restores the saved level")

            feat.reset_mock()
            asleep = fnkeys.Keyboard(Path("/dev/hidraw-test"), lid_closed=False, sleeping=True)
            check(asleep.backlight == 3, "starting during suspend keeps the saved level")
            check(
                feat.call_args.args[1] == fnkeys.BACKLIGHT[0],
                "starting during suspend writes backlight 0",
            )

    power = fnkeys.PowerEvents()
    seen: dict[str, object] = {}
    read_fd, inhibit_fd = os.pipe()

    class Probe:
        def set_power_state(self, lid_closed: bool, sleeping: bool) -> None:
            try:
                fcntl.fcntl(inhibit_fd, fcntl.F_GETFD)
                seen["inhibit_open"] = True
            except OSError:
                seen["inhibit_open"] = False
            seen["state"] = (lid_closed, sleeping)

    power._kbd = Probe()
    power._inhibit_fd = inhibit_fd
    power.note_sleep(True)
    check(seen["state"] == (False, True), "suspend blanks before the lid flag matters")
    check(seen["inhibit_open"] is True, "backlight goes off while the sleep delay lock is still held")
    try:
        fcntl.fcntl(inhibit_fd, fcntl.F_GETFD)
        released = False
    except OSError:
        released = True
    check(released, "sleep delay lock is released after the lamp is off")
    os.close(read_fd)

    power.note_lid(True)
    check(power.lid_closed is True and power.sleeping is True, "lid close during suspend keeps both flags")
    power.note_sleep(False)
    check(seen["state"] == (True, False), "resume with the lid shut stays blanked")
    power.note_lid(False)
    check(seen["state"] == (False, False), "opening the lid after resume restores the lamp")

    woke = fnkeys.PowerEvents()
    woke.lid_closed = True
    woke.sleeping = True
    woke._query_lid = lambda: False  # type: ignore[method-assign]
    woke._kbd = Probe()
    woke.note_sleep(False)
    check(seen["state"] == (False, False), "resume re-reads an open lid and restores the lamp")


def test_handle_report() -> None:
    kbd = MagicMock()
    talk = MagicMock()

    fnkeys.handle_report(b"\x5a\x3d\x64\x01\x00\x00", kbd, talk)
    check(not kbd.mock_calls and not talk.mock_calls, "battery keepalive is ignored")

    fnkeys.handle_report(b"\x5a\x00", kbd, talk)
    check(talk.stop.call_args == (("fn9",),), "release stops dictation")

    fnkeys.handle_report(b"\x5a\xc7", kbd, talk)
    check(kbd.cycle_backlight.called, "F4 cycles keyboard backlight")

    fnkeys.handle_report(b"\x5a\x4e", kbd, talk)
    check(kbd.toggle_fn_lock.called, "Fn+Esc toggles fn-lock")

    with patch.object(fnkeys, "brightness_step") as step:
        fnkeys.handle_report(b"\x5a\x10", kbd, talk)
        fnkeys.handle_report(b"\x5a\x20", kbd, talk)
        check(step.call_args_list == [(("down",),), (("up",),)], "brightness keys call omarchy")

    with patch.object(fnkeys, "toggle_emojis") as emoji:
        fnkeys.handle_report(b"\x5a\x7e", kbd, talk)
        check(emoji.called, "emoji key opens picker")

    talk.start.reset_mock()
    fnkeys.handle_report(b"\x5a\x7c", kbd, talk)
    check(talk.start.call_args == (("fn9",),), "mic key starts hold-to-talk")


def main() -> int:
    test_udev_rules()
    test_vendor_page()
    test_lid_and_sleep_blank_backlight()
    test_handle_report()
    if fail:
        print("FAILED")
        return 1
    print("PASSED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
