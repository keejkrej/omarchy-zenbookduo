//! Colours from the current Omarchy theme, and the window frame (gaps,
//! border, rounding) from Hyprland, so the deck looks like the windows
//! around it.
//!
//! Omarchy writes the active palette to
//! ~/.local/state/omarchy/current/theme/colors.toml; a theme switch rewrites
//! it, and `changed` notices that from the file's modification time.

use std::path::PathBuf;
use std::process::Command;
use std::sync::RwLock;
use std::time::SystemTime;

use iced::Color;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub deck: Color,
    pub pad: Color,
    pub key: Color,
    pub pressed: Color,
    pub lit: Color,
    pub legend: Color,
    /// Third-level (AltGr) legends.
    pub alt_legend: Color,
    pub border: Color,
}

/// The original look, for when there is no Omarchy theme.
const FALLBACK: Palette = Palette {
    deck: Color::from_rgb8(0x56, 0x57, 0x5b),
    pad: Color::from_rgb8(0x4b, 0x4c, 0x50),
    key: Color::from_rgb8(0x35, 0x36, 0x39),
    pressed: Color::from_rgb8(0x6c, 0x6e, 0x74),
    lit: Color::from_rgb8(0x3a, 0x5d, 0xa8),
    legend: Color::from_rgb8(0xd9, 0xda, 0xdc),
    alt_legend: Color::from_rgb8(0x9a, 0xb8, 0xf0),
    border: Color::from_rgb8(0x3a, 0x5d, 0xa8),
};

static PALETTE: RwLock<Palette> = RwLock::new(FALLBACK);

pub fn palette() -> Palette {
    *PALETTE.read().unwrap_or_else(|e| e.into_inner())
}

fn colors_file() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"))
        .join("omarchy/current/theme/colors.toml")
}

fn hex(value: &str) -> Option<Color> {
    let v = value.trim().trim_matches('"').strip_prefix('#')?;
    if v.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(v, 16).ok()?;
    Some(Color::from_rgb8((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

/// `key = "#rrggbb"` lines of colors.toml.
fn read_colors() -> Option<std::collections::HashMap<String, Color>> {
    let text = std::fs::read_to_string(colors_file()).ok()?;
    let colors = text
        .lines()
        .filter_map(|l| l.split_once('='))
        .filter_map(|(k, v)| Some((k.trim().to_string(), hex(v)?)))
        .collect::<std::collections::HashMap<_, _>>();
    (!colors.is_empty()).then_some(colors)
}

/// Load the theme's palette. Keys sit on the lighter background like
/// Omarchy's panels, latched modifiers and the frame take the accent.
pub fn load() {
    let Some(c) = read_colors() else { return };
    let get = |names: &[&str], fallback: Color| names.iter().find_map(|n| c.get(*n).copied()).unwrap_or(fallback);
    let accent = get(&["accent", "blue"], FALLBACK.lit);
    let palette = Palette {
        deck: get(&["background"], FALLBACK.deck),
        pad: get(&["dark_background", "background"], FALLBACK.pad),
        key: get(&["lighter_background", "selection"], FALLBACK.key),
        pressed: get(&["muted", "selection"], FALLBACK.pressed),
        lit: accent,
        legend: get(&["foreground"], FALLBACK.legend),
        alt_legend: accent,
        border: accent,
    };
    *PALETTE.write().unwrap_or_else(|e| e.into_inner()) = palette;
}

/// Modification time of colors.toml, to spot a theme switch.
pub fn stamp() -> Option<SystemTime> {
    std::fs::metadata(colors_file()).and_then(|m| m.modified()).ok()
}

/// Hyprland's window frame, in logical pixels.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub gaps: i32,
    pub border: f32,
    pub rounding: f32,
}

fn hypr_option(name: &str) -> Option<String> {
    let out = Command::new("hyprctl").args(["getoption", name, "-j"]).output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// First number after `"key":` in hyprctl's JSON (gaps come as a CSS-style
/// "10 10 10 10" string; one value is enough for the deck).
fn json_number(json: &str, key: &str) -> Option<f32> {
    let rest = json.split_once(&format!("\"{key}\":"))?.1.trim_start().trim_start_matches('"');
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

pub fn frame() -> Frame {
    let num = |name: &str, key: &str| hypr_option(name).and_then(|j| json_number(&j, key));
    Frame {
        gaps: num("general:gaps_out", "css").unwrap_or(0.0) as i32,
        border: num("general:border_size", "int").unwrap_or(0.0),
        rounding: num("decoration:rounding", "int").unwrap_or(0.0),
    }
}
