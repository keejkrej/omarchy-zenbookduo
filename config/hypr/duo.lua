-- ASUS Zenbook Duo (UX8406) helpers for Hyprland on Omarchy.
-- Keyboard snap is the USB pogo-pin connection (0b05:1bf2 on UX8406CA,
-- 0b05:1b2c on UX8406MA), not Bluetooth.

local M = {}

function M.keyboard_snapped()
  local handle = io.popen([[
    for d in /sys/bus/usb/devices/*; do
      if [ -f "$d/idVendor" ] && [ -f "$d/idProduct" ]; then
        v=$(cat "$d/idVendor")
        p=$(cat "$d/idProduct")
        if [ "$v" = "0b05" ]; then
          case "$p" in
            1bf2|1b2c) echo 1; break ;;
          esac
        fi
      fi
    done
  ]])
  if not handle then
    return false
  end
  local out = handle:read("*a") or ""
  handle:close()
  return out:find("1", 1, true) ~= nil
end

-- Top panel eDP-1, bottom panel eDP-2 stacked underneath when the keyboard
-- is off the glass. Disable eDP-2 while the keyboard is snapped on.
function M.apply_monitors(scale)
  hl.monitor({ output = "eDP-1", mode = "preferred", position = "0x0", scale = scale })

  if M.keyboard_snapped() then
    hl.monitor({ output = "eDP-2", disabled = true })
  else
    hl.monitor({ output = "eDP-2", mode = "preferred", position = "auto-down", scale = scale })
  end
end

-- Map each digitizer to the panel it sits on. Inhibit the bottom digitizer
-- while the keyboard is covering that panel.
-- UX8406CA uses 04f3:4445/4446; UX8406MA uses 04f3:425b/425a.
function M.apply_devices()
  local snapped = M.keyboard_snapped()
  local panels = {
    { prefix = "elan9008:00-04f3:4445", output = "eDP-1" },
    { prefix = "elan9009:00-04f3:4446", output = "eDP-2" },
    { prefix = "elan9008:00-04f3:425b", output = "eDP-1" },
    { prefix = "elan9009:00-04f3:425a", output = "eDP-2" },
  }

  for _, panel in ipairs(panels) do
    local enabled = panel.output ~= "eDP-2" or not snapped
    for _, suffix in ipairs({ "", "-stylus", "-touchpad" }) do
      hl.device({
        name = panel.prefix .. suffix,
        output = panel.output,
        enabled = enabled,
      })
    end
  end
end

return M
