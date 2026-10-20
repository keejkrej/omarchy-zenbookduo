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

local function state_path(name)
  local base = os.getenv("XDG_STATE_HOME") or (os.getenv("HOME") .. "/.local/state")
  return base .. "/zenbook/" .. name
end

-- Device orientation written by zenbook-duo-rotate-watch (normal, bottom-up,
-- left-up, right-up as iio-sensor-proxy names them, or sharing).
function M.orientation()
  local f = io.open(state_path("rotation"), "r")
  if not f then
    return "normal"
  end
  local value = (f:read("*l") or "normal"):match("^%s*(.-)%s*$")
  f:close()
  return value
end

-- True while the on-screen keyboard (zenbook-duo-keyboard) is running. It
-- writes its pid; a dead pid left behind by pkill does not count.
function M.osk_up()
  local f = io.open(state_path("osk.pid"), "r")
  if not f then
    return false
  end
  local pid = (f:read("*l") or ""):match("%d+")
  f:close()
  -- Check the command line, not comm: comm is cut to 15 characters, which
  -- zenbook-duo-keyboard-watch shares.
  local cmdline = pid and io.open("/proc/" .. pid .. "/cmdline", "r")
  if not cmdline then
    return false
  end
  local argv0 = (cmdline:read("*a") or ""):match("^[^%z]*")
  cmdline:close()
  return argv0:match("zenbook%-duo%-keyboard$") ~= nil
end

-- Panel transforms for the current orientation, read by apply_devices so
-- touch input turns with its panel.
M.transform_top, M.transform_bottom = 0, 0

-- Top panel eDP-1, bottom panel eDP-2 stacked underneath when the keyboard
-- is off the glass. Disable eDP-2 while the keyboard is snapped on.
-- layout.top and layout.bottom are logical-pixel positions. Defaults keep
-- the pair at the origin, with the bottom panel directly underneath. When
-- the device is rotated, the rotated layout replaces those positions.
-- While the on-screen keyboard is up, a 100 px gap keeps the cursor off the
-- bottom panel and workspace 2 is not pinned to it.
function M.apply_monitors(scale, layout)
  layout = layout or {}
  local h = math.floor(1800 / scale) -- panel height; also the width once rotated
  local gap = M.osk_up() and 100 or 0
  local off = h + gap
  local layouts = {
    ["normal"] = { 0, layout.top or "0x0", 0, gap == 0 and layout.bottom or ("0x" .. off) },
    ["bottom-up"] = { 2, "0x" .. off, 2, "0x0" },
    ["left-up"] = { 1, off .. "x0", 1, "0x0" },
    ["right-up"] = { 3, "0x0", 3, off .. "x0" },
    -- Opened flat on a table: turn only the top panel for the person opposite.
    ["sharing"] = { 2, "0x0", 0, "0x" .. off },
  }
  local l = layouts[M.orientation()] or layouts["normal"]
  M.transform_top, M.transform_bottom = l[1], l[3]

  hl.monitor({ output = "eDP-1", mode = "preferred", position = l[2], scale = scale, transform = l[1] })

  if M.keyboard_snapped() then
    hl.monitor({ output = "eDP-2", disabled = true })
  else
    -- disabled = false is required: Hyprland 0.55+ can leave a previously
    -- disabled output in the layout with DPMS off (black panel).
    hl.monitor({
      output = "eDP-2",
      mode = "preferred",
      position = l[4] or "auto-down",
      scale = scale,
      transform = l[3],
      disabled = false,
    })
    if gap == 0 then
      hl.workspace_rule({ workspace = "2", monitor = "eDP-2", default = true })
    end
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
        transform = panel.output == "eDP-1" and M.transform_top or M.transform_bottom,
      })
    end
  end

  M.apply_input()
end

-- Disable-while-typing plus drop the relative mouse node. That node has no
-- palm detection; the actual touchpad is *-keyboard-touchpad.
function M.apply_input()
  hl.config({
    input = {
      touchpad = {
        disable_while_typing = true,
      },
    },
    -- The on-screen touchpad is a touchscreen; hiding the cursor on touch
    -- would make it vanish whenever the touchpad is used.
    cursor = {
      hide_on_touch = false,
    },
  })

  hl.device({
    name = "primax-electronics-ltd.-asus-zenbook-duo-keyboard-mouse",
    enabled = false,
  })
  -- Bluetooth node name omits the Primax USB prefix.
  hl.device({
    name = "asus-zenbook-duo-keyboard-mouse",
    enabled = false,
  })
end

return M
