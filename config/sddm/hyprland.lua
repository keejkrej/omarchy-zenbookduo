-- SDDM greeter on the Zenbook Duo: Omarchy's greeter config, on the top
-- panel only. The greeter puts a login view on every screen, and with the
-- keyboard snapped onto the bottom panel, typing could land in the hidden
-- view's password field.
dofile("/usr/share/sddm/hyprland.lua")

hl.monitor({ output = "eDP-2", disabled = true })
