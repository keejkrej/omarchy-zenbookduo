-- ASUS Zenbook Duo (UX8406) display layout for Omarchy/Hyprland.
-- See https://wiki.hypr.land/Configuring/Basics/Monitors/

local omarchy_gdk_scale = 1.6
local omarchy_monitor_scale = 1.6
local duo = require("hypr.duo")

hl.env("GDK_SCALE", tostring(omarchy_gdk_scale))
duo.apply_monitors(omarchy_monitor_scale)

-- External monitors.
hl.monitor({ output = "", mode = "preferred", position = "auto", scale = omarchy_monitor_scale })
