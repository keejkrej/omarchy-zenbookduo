//! On-screen keyboard and touchpad for the Zenbook Duo's bottom screen,
//! drawn after the detachable keyboard (key positions are measured from a
//! photo of it, in that photo's pixels).
//!
//! It is a wlr layer-shell surface filling eDP-2: it gets touch and pointer
//! input but never keyboard focus, so keys type into the window you were
//! using. Keys and touchpad go out through a persistent virtual keyboard and
//! virtual pointer (see input.rs). No root needed.

mod input;
mod theme;

use std::collections::HashMap;
use std::process::Command;
use std::sync::OnceLock;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use iced::alignment::Vertical;
use iced::mouse;
use iced::touch;
use iced::widget::canvas::{self, Frame, Path, Text};
use iced::widget::text::Alignment;
use iced::{Color, Element, Event, Font, Length, Pixels, Point, Rectangle, Size, Theme};
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
use iced_layershell::to_layer_message;

use input::InputCmd;

const OUTPUT: &str = "eDP-2";
const ICONS: Font = Font::with_name("JetBrainsMono Nerd Font");

/// How far each icon's ink is off its advance's centre, in ems. The Nerd
/// Font's icons are wider than its monospace cell and start at the cell's
/// left edge, so centring the cell leaves wide icons off to the right.
fn icon_offset(c: char) -> f32 {
    static OFFSETS: OnceLock<HashMap<char, f32>> = OnceLock::new();
    let offsets = OFFSETS.get_or_init(|| {
        let mut map = HashMap::new();
        let Some(data) = Command::new("fc-match")
            .args(["-f", "%{file}", "JetBrainsMono Nerd Font"])
            .output()
            .ok()
            .and_then(|o| std::fs::read(String::from_utf8_lossy(&o.stdout).as_ref()).ok())
        else {
            return map;
        };
        let Ok(face) = ttf_parser::Face::parse(&data, 0) else { return map };
        let em = f32::from(face.units_per_em());
        let icons = rows().iter().flat_map(|r| r.keys.iter().map(|k| &k.legend))
            .chain(bottom_keys().iter().map(|(_, _, k)| &k.legend))
            .chain(ARROWS.iter().map(|(_, k)| &k.legend));
        for legend in icons {
            let Legend::Icon(c, _) = *legend else { continue };
            let Some(g) = face.glyph_index(c) else { continue };
            let (Some(b), Some(adv)) = (face.glyph_bounding_box(g), face.glyph_hor_advance(g)) else { continue };
            let ink = (f32::from(b.x_min) + f32::from(b.x_max)) / 2.0;
            map.insert(c, (ink - f32::from(adv) / 2.0) / em);
        }
        map
    });
    offsets.get(&c).copied().unwrap_or(0.0)
}

// --- Layout ---------------------------------------------------------------

/// The keyboard deck in the photo (pixels): everything is drawn relative to it.
const DECK: (f32, f32, f32, f32) = (145.0, 165.0, 1457.0, 995.0);
/// The touchpad, in photo pixels (left, top, right, bottom).
const PAD: (f32, f32, f32, f32) = (530.0, 625.0, 1070.0, 957.0);
/// Space between keys, in photo pixels.
const GAP: f32 = 11.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// Printable key: keysym, plain and shifted character.
    Char(&'static str, char, char),
    /// Non-printable key; with Fn latched it sends the second keysym.
    Key(&'static str, &'static str),
    /// F-row key: media keysym, and the F-key Fn selects.
    FRow(&'static str, &'static str),
    /// Send a fixed chord (modifier mask, keysym), e.g. the Copilot key.
    Chord(u32, &'static str),
    Mod(Modifier),
    Caps,
    Fn,
    Hide,
    SwapScreens,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modifier {
    Shift,
    Ctrl,
    Alt,
    AltGr,
    Super,
}

impl Modifier {
    fn mask(self) -> u32 {
        match self {
            Modifier::Shift => input::MOD_SHIFT,
            Modifier::Ctrl => input::MOD_CTRL,
            Modifier::Alt => input::MOD_ALT,
            Modifier::AltGr => input::MOD_ALTGR,
            Modifier::Super => input::MOD_SUPER,
        }
    }
}

/// What is printed on a key.
#[derive(Debug, Clone, Copy)]
enum Legend {
    /// One large label (letters).
    Main(&'static str),
    /// Shifted symbol above, plain symbol below (number row, punctuation).
    Dual(&'static str, &'static str),
    /// As Dual, plus the AltGr symbol at the lower right (Nordic layouts).
    Triple(&'static str, &'static str, &'static str),
    /// Small text label (Ctrl, Caps, …).
    Word(&'static str),
    /// An icon, optionally with a small F-key name beside it.
    Icon(char, &'static str),
    Blank,
}

struct KeyDef {
    units: f32,
    legend: Legend,
    action: Action,
}

const fn key(units: f32, legend: Legend, action: Action) -> KeyDef {
    KeyDef { units, legend, action }
}
const fn letter(l: &'static str, sym: &'static str, plain: char, upper: char) -> KeyDef {
    key(1.0, Legend::Main(l), Action::Char(sym, plain, upper))
}
const fn dual(top: &'static str, main: &'static str, sym: &'static str, plain: char, shifted: char) -> KeyDef {
    key(1.0, Legend::Dual(top, main), Action::Char(sym, plain, shifted))
}
const fn triple(top: &'static str, main: &'static str, alt: &'static str, sym: &'static str, plain: char, shifted: char) -> KeyDef {
    key(1.0, Legend::Triple(top, main, alt), Action::Char(sym, plain, shifted))
}
const fn frow(icon: char, f: &'static str, media: &'static str) -> KeyDef {
    key(1.0, Legend::Icon(icon, f), Action::FRow(media, f))
}

/// A row: left, right, top, bottom (photo pixels) and its keys. Key widths
/// are in units; the row's width is shared out in proportion.
struct RowDef {
    span: (f32, f32, f32, f32),
    keys: &'static [KeyDef],
}

const US_ROWS: &[RowDef] = &[
    RowDef {
        span: (230.0, 1368.0, 206.0, 241.0),
        keys: &[
            key(1.0, Legend::Icon('\u{f033e}', "esc"), Action::Key("Escape", "Escape")),
            frow('\u{f0581}', "F1", "XF86AudioMute"),
            frow('\u{f075e}', "F2", "XF86AudioLowerVolume"),
            frow('\u{f075d}', "F3", "XF86AudioRaiseVolume"),
            frow('\u{f0313}', "F4", "XF86KbdLightOnOff"),
            frow('\u{f00de}', "F5", "XF86MonBrightnessDown"),
            frow('\u{f00e0}', "F6", "XF86MonBrightnessUp"),
            frow('\u{f037a}', "F7", "XF86Display"),
            frow('\u{f0e51}', "F8", "Print"),
            key(1.0, Legend::Word("F9"), Action::FRow("F9", "F9")),
            frow('\u{f036d}', "F10", "XF86AudioMicMute"),
            key(1.0, Legend::Word("F11"), Action::FRow("F11", "F11")),
            key(1.0, Legend::Word("/A  F12"), Action::FRow("F12", "F12")),
            key(1.0, Legend::Icon('\u{f030f}', ""), Action::Hide),
            key(1.0, Legend::Icon('\u{f04e1}', ""), Action::SwapScreens),
            key(1.0, Legend::Word("DEL INS"), Action::Key("Delete", "Insert")),
        ],
    },
    RowDef {
        span: (230.0, 1368.0, 250.0, 310.0),
        keys: &[
            dual("~", "`", "grave", '`', '~'),
            dual("!", "1", "1", '1', '!'),
            dual("@", "2", "2", '2', '@'),
            dual("#", "3", "3", '3', '#'),
            dual("$", "4", "4", '4', '$'),
            dual("%", "5", "5", '5', '%'),
            dual("^", "6", "6", '6', '^'),
            dual("&", "7", "7", '7', '&'),
            dual("*", "8", "8", '8', '*'),
            dual("(", "9", "9", '9', '('),
            dual(")", "0", "0", '0', ')'),
            dual("_", "-", "minus", '-', '_'),
            dual("+", "=", "equal", '=', '+'),
            key(1.66, Legend::Main("←"), Action::Key("BackSpace", "BackSpace")),
        ],
    },
    RowDef {
        span: (228.0, 1370.0, 322.0, 385.0),
        keys: &[
            key(1.67, Legend::Icon('\u{f0312}', ""), Action::Key("Tab", "Tab")),
            letter("Q", "q", 'q', 'Q'),
            letter("W", "w", 'w', 'W'),
            letter("E", "e", 'e', 'E'),
            letter("R", "r", 'r', 'R'),
            letter("T", "t", 't', 'T'),
            letter("Y", "y", 'y', 'Y'),
            letter("U", "u", 'u', 'U'),
            letter("I", "i", 'i', 'I'),
            letter("O", "o", 'o', 'O'),
            letter("P", "p", 'p', 'P'),
            dual("{", "[", "bracketleft", '[', '{'),
            dual("}", "]", "bracketright", ']', '}'),
            key(1.15, Legend::Dual("|", "\\"), Action::Char("backslash", '\\', '|')),
        ],
    },
    RowDef {
        span: (226.0, 1372.0, 395.0, 458.0),
        keys: &[
            key(1.9, Legend::Word("CAPS"), Action::Caps),
            letter("A", "a", 'a', 'A'),
            letter("S", "s", 's', 'S'),
            letter("D", "d", 'd', 'D'),
            letter("F", "f", 'f', 'F'),
            letter("G", "g", 'g', 'G'),
            letter("H", "h", 'h', 'H'),
            letter("J", "j", 'j', 'J'),
            letter("K", "k", 'k', 'K'),
            letter("L", "l", 'l', 'L'),
            dual(":", ";", "semicolon", ';', ':'),
            dual("\"", "'", "apostrophe", '\'', '"'),
            key(1.93, Legend::Icon('\u{f0311}', ""), Action::Key("Return", "Return")),
        ],
    },
    RowDef {
        span: (223.0, 1375.0, 470.0, 533.0),
        keys: &[
            key(2.45, Legend::Icon('\u{f0636}', ""), Action::Mod(Modifier::Shift)),
            letter("Z", "z", 'z', 'Z'),
            letter("X", "x", 'x', 'X'),
            letter("C", "c", 'c', 'C'),
            letter("V", "v", 'v', 'V'),
            letter("B", "b", 'b', 'B'),
            letter("N", "n", 'n', 'N'),
            letter("M", "m", 'm', 'M'),
            dual("<", ",", "comma", ',', '<'),
            dual(">", ".", "period", '.', '>'),
            dual("?", "/", "slash", '/', '?'),
            key(2.45, Legend::Icon('\u{f0636}', ""), Action::Mod(Modifier::Shift)),
        ],
    },
];

/// Finnish/Swedish ISO layout on the same deck. Keysyms name the physical
/// key (by its US-layout name, see input.rs); the fi keymap gives it its
/// Nordic characters. There is no room for ISO's L-shaped Enter, so the
/// '* key sits where the US backslash key is.
const FI_ROWS: &[RowDef] = &[
    RowDef {
        span: (230.0, 1368.0, 206.0, 241.0),
        keys: &[
            key(1.0, Legend::Icon('\u{f033e}', "esc"), Action::Key("Escape", "Escape")),
            frow('\u{f0581}', "F1", "XF86AudioMute"),
            frow('\u{f075e}', "F2", "XF86AudioLowerVolume"),
            frow('\u{f075d}', "F3", "XF86AudioRaiseVolume"),
            frow('\u{f0313}', "F4", "XF86KbdLightOnOff"),
            frow('\u{f00de}', "F5", "XF86MonBrightnessDown"),
            frow('\u{f00e0}', "F6", "XF86MonBrightnessUp"),
            frow('\u{f037a}', "F7", "XF86Display"),
            frow('\u{f0e51}', "F8", "Print"),
            key(1.0, Legend::Word("F9"), Action::FRow("F9", "F9")),
            frow('\u{f036d}', "F10", "XF86AudioMicMute"),
            key(1.0, Legend::Word("F11"), Action::FRow("F11", "F11")),
            key(1.0, Legend::Word("/A  F12"), Action::FRow("F12", "F12")),
            key(1.0, Legend::Icon('\u{f030f}', ""), Action::Hide),
            key(1.0, Legend::Icon('\u{f04e1}', ""), Action::SwapScreens),
            key(1.0, Legend::Word("DEL INS"), Action::Key("Delete", "Insert")),
        ],
    },
    RowDef {
        span: (230.0, 1368.0, 250.0, 310.0),
        keys: &[
            dual("½", "§", "grave", '§', '½'),
            dual("!", "1", "1", '1', '!'),
            triple("\"", "2", "@", "2", '2', '"'),
            triple("#", "3", "£", "3", '3', '#'),
            triple("¤", "4", "$", "4", '4', '¤'),
            triple("%", "5", "€", "5", '5', '%'),
            dual("&", "6", "6", '6', '&'),
            triple("/", "7", "{", "7", '7', '/'),
            triple("(", "8", "[", "8", '8', '('),
            triple(")", "9", "]", "9", '9', ')'),
            triple("=", "0", "}", "0", '0', '='),
            triple("?", "+", "\\", "minus", '+', '?'),
            dual("`", "´", "equal", '´', '`'),
            key(1.66, Legend::Main("←"), Action::Key("BackSpace", "BackSpace")),
        ],
    },
    RowDef {
        span: (228.0, 1370.0, 322.0, 385.0),
        keys: &[
            key(1.67, Legend::Icon('\u{f0312}', ""), Action::Key("Tab", "Tab")),
            letter("Q", "q", 'q', 'Q'),
            letter("W", "w", 'w', 'W'),
            key(1.0, Legend::Triple("", "E", "€"), Action::Char("e", 'e', 'E')),
            letter("R", "r", 'r', 'R'),
            letter("T", "t", 't', 'T'),
            letter("Y", "y", 'y', 'Y'),
            letter("U", "u", 'u', 'U'),
            letter("I", "i", 'i', 'I'),
            letter("O", "o", 'o', 'O'),
            letter("P", "p", 'p', 'P'),
            letter("Å", "bracketleft", 'å', 'Å'),
            triple("^", "¨", "~", "bracketright", '¨', '^'),
            key(1.15, Legend::Dual("*", "'"), Action::Char("backslash", '\'', '*')),
        ],
    },
    RowDef {
        span: (226.0, 1372.0, 395.0, 458.0),
        keys: &[
            key(1.9, Legend::Word("CAPS"), Action::Caps),
            letter("A", "a", 'a', 'A'),
            letter("S", "s", 's', 'S'),
            letter("D", "d", 'd', 'D'),
            letter("F", "f", 'f', 'F'),
            letter("G", "g", 'g', 'G'),
            letter("H", "h", 'h', 'H'),
            letter("J", "j", 'j', 'J'),
            letter("K", "k", 'k', 'K'),
            letter("L", "l", 'l', 'L'),
            letter("Ö", "semicolon", 'ö', 'Ö'),
            letter("Ä", "apostrophe", 'ä', 'Ä'),
            key(1.93, Legend::Icon('\u{f0311}', ""), Action::Key("Return", "Return")),
        ],
    },
    RowDef {
        span: (223.0, 1375.0, 470.0, 533.0),
        keys: &[
            key(1.35, Legend::Icon('\u{f0636}', ""), Action::Mod(Modifier::Shift)),
            triple(">", "<", "|", "less", '<', '>'),
            letter("Z", "z", 'z', 'Z'),
            letter("X", "x", 'x', 'X'),
            letter("C", "c", 'c', 'C'),
            letter("V", "v", 'v', 'V'),
            letter("B", "b", 'b', 'B'),
            letter("N", "n", 'n', 'N'),
            letter("M", "m", 'm', 'M'),
            dual(";", ",", "comma", ',', ';'),
            dual(":", ".", "period", '.', ':'),
            dual("_", "-", "slash", '-', '_'),
            key(2.45, Legend::Icon('\u{f0636}', ""), Action::Mod(Modifier::Shift)),
        ],
    },
];

/// Bottom row, given key by key in photo pixels (left, right) because of
/// the half-height arrow cluster. Top and bottom of the row:
const BOTTOM_ROW: (f32, f32) = (543.0, 607.0);
const US_BOTTOM_KEYS: &[(f32, f32, KeyDef)] = &[
    (221.0, 321.0, key(0.0, Legend::Word("CTRL"), Action::Mod(Modifier::Ctrl))),
    (332.0, 400.0, key(0.0, Legend::Word("FN"), Action::Fn)),
    (410.0, 480.0, key(0.0, Legend::Icon('\u{f05b3}', ""), Action::Mod(Modifier::Super))),
    (490.0, 558.0, key(0.0, Legend::Word("ALT"), Action::Mod(Modifier::Alt))),
    (568.0, 950.0, key(0.0, Legend::Blank, Action::Char("space", ' ', ' '))),
    (960.0, 1027.0, key(0.0, Legend::Word("ALT"), Action::Mod(Modifier::Alt))),
    (1038.0, 1107.0, key(0.0, Legend::Icon('\u{f0674}', ""), Action::Chord(input::MOD_SHIFT | input::MOD_SUPER, "F23"))),
];
/// Nordic keyboards have AltGr on the right.
const FI_BOTTOM_KEYS: &[(f32, f32, KeyDef)] = &[
    (221.0, 321.0, key(0.0, Legend::Word("CTRL"), Action::Mod(Modifier::Ctrl))),
    (332.0, 400.0, key(0.0, Legend::Word("FN"), Action::Fn)),
    (410.0, 480.0, key(0.0, Legend::Icon('\u{f05b3}', ""), Action::Mod(Modifier::Super))),
    (490.0, 558.0, key(0.0, Legend::Word("ALT"), Action::Mod(Modifier::Alt))),
    (568.0, 950.0, key(0.0, Legend::Blank, Action::Char("space", ' ', ' '))),
    (960.0, 1027.0, key(0.0, Legend::Word("ALT GR"), Action::Mod(Modifier::AltGr))),
    (1038.0, 1107.0, key(0.0, Legend::Icon('\u{f0674}', ""), Action::Chord(input::MOD_SHIFT | input::MOD_SUPER, "F23"))),
];

/// Keyboard layout the deck shows and types with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layout {
    Us,
    Fi,
}

/// Follow Hyprland's first keyboard layout, or ZENBOOK_DUO_OSK_LAYOUT (us,
/// fi). Finnish and Swedish share the deck; anything else gets US.
fn layout() -> Layout {
    static LAYOUT: OnceLock<Layout> = OnceLock::new();
    *LAYOUT.get_or_init(|| {
        let name = std::env::var("ZENBOOK_DUO_OSK_LAYOUT").ok().or_else(|| {
            let out = Command::new("hyprctl").args(["getoption", "input:kb_layout", "-j"]).output().ok()?;
            let json = String::from_utf8_lossy(&out.stdout).into_owned();
            let value = json.split_once("\"str\":")?.1.trim_start().strip_prefix('"')?.split('"').next()?.to_string();
            Some(value)
        });
        match name.as_deref().and_then(|n| n.split(',').next()).map(str::trim) {
            Some("fi" | "se") => Layout::Fi,
            _ => Layout::Us,
        }
    })
}

fn rows() -> &'static [RowDef] {
    match layout() {
        Layout::Us => US_ROWS,
        Layout::Fi => FI_ROWS,
    }
}

fn bottom_keys() -> &'static [(f32, f32, KeyDef)] {
    match layout() {
        Layout::Us => US_BOTTOM_KEYS,
        Layout::Fi => FI_BOTTOM_KEYS,
    }
}

/// XKB layout for the virtual keyboard's keymap.
fn xkb_layout() -> &'static str {
    match layout() {
        Layout::Us => "us",
        Layout::Fi => "fi",
    }
}
/// Arrow cluster: (left, right, top, bottom) in photo pixels.
const ARROWS: &[((f32, f32, f32, f32), KeyDef)] = &[
    ((1118.0, 1192.0, 577.0, 607.0), key(0.0, Legend::Icon('\u{f035e}', ""), Action::Key("Left", "Home"))),
    ((1203.0, 1290.0, 543.0, 573.0), key(0.0, Legend::Icon('\u{f0360}', ""), Action::Key("Up", "Prior"))),
    ((1203.0, 1290.0, 577.0, 607.0), key(0.0, Legend::Icon('\u{f035d}', ""), Action::Key("Down", "Next"))),
    ((1302.0, 1380.0, 577.0, 607.0), key(0.0, Legend::Icon('\u{f035f}', ""), Action::Key("Right", "End"))),
];

/// A key placed on the deck: rectangle in deck fractions (0..1).
struct Placed {
    rect: Rectangle,
    legend: Legend,
    action: Action,
}

fn frac(l: f32, t: f32, r: f32, b: f32) -> Rectangle {
    let (dl, dt, dr, db) = DECK;
    let (w, h) = (dr - dl, db - dt);
    Rectangle {
        x: (l - dl) / w,
        y: (t - dt) / h,
        width: (r - l) / w,
        height: (b - t) / h,
    }
}

fn build_layout() -> Vec<Placed> {
    let mut keys = Vec::new();
    for row in rows() {
        let (left, right, top, bottom) = row.span;
        let units: f32 = row.keys.iter().map(|k| k.units).sum();
        let unit = (right - left + GAP) / units;
        let mut x = left;
        for k in row.keys {
            let w = k.units * unit - GAP;
            keys.push(Placed { rect: frac(x, top, x + w, bottom), legend: k.legend, action: k.action });
            x += w + GAP;
        }
    }
    for (l, r, k) in bottom_keys() {
        keys.push(Placed { rect: frac(*l, BOTTOM_ROW.0, *r, BOTTOM_ROW.1), legend: k.legend, action: k.action });
    }
    for ((l, r, t, b), k) in ARROWS {
        keys.push(Placed { rect: frac(*l, *t, *r, *b), legend: k.legend, action: k.action });
    }
    keys
}

// --- State ----------------------------------------------------------------

#[to_layer_message]
#[derive(Debug, Clone)]
enum Message {
    KeyDown(usize),
    KeyUp(usize),
    Tick(Instant),
    Input(InputCmd),
    /// Look for a theme switch.
    ThemeCheck,
}

struct Keyboard {
    keys: Vec<Placed>,
    held: Vec<usize>,
    /// Modifiers latched for the next key (released after it).
    latched: Vec<Modifier>,
    caps: bool,
    fn_lock: bool,
    output: Sender<InputCmd>,
    /// The deck as drawn with nothing pressed; only Fn changes it.
    deck: canvas::Cache,
    /// Key being held for auto-repeat: index, pressed at, last repeat.
    repeat: Option<(usize, Instant, Instant)>,
    /// Hyprland's window frame, drawn around the deck.
    frame: theme::Frame,
    /// colors.toml's modification time when the palette was loaded.
    theme_stamp: Option<std::time::SystemTime>,
}

/// Hold this long before a key repeats, then repeat at this interval
/// (typical keyboard settings: 500 ms, ~30 per second).
const REPEAT_DELAY: Duration = Duration::from_millis(500);
const REPEAT_EVERY: Duration = Duration::from_millis(33);

impl Keyboard {
    fn new() -> Self {
        Keyboard {
            keys: build_layout(),
            held: Vec::new(),
            latched: Vec::new(),
            caps: false,
            fn_lock: false,
            output: input::spawn(xkb_layout()),
            deck: canvas::Cache::new(),
            repeat: None,
            frame: theme::frame(),
            theme_stamp: theme::stamp(),
        }
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::KeyDown(i) => {
                self.held.push(i);
                let action = self.keys[i].action;
                if matches!(action, Action::Char(..) | Action::Key(..) | Action::FRow(..)) {
                    let now = Instant::now();
                    self.repeat = Some((i, now, now));
                }
                return self.press(action);
            }
            Message::KeyUp(i) => {
                self.held.retain(|h| *h != i);
                if self.repeat.is_some_and(|(r, ..)| r == i) {
                    self.repeat = None;
                }
            }
            Message::Tick(now) => {
                if let Some((i, since, last)) = self.repeat
                    && now.duration_since(since) >= REPEAT_DELAY
                    && now.duration_since(last) >= REPEAT_EVERY
                {
                    self.repeat = Some((i, since, now));
                    return self.press(self.keys[i].action);
                }
            }
            Message::Input(cmd) => {
                let _ = self.output.send(cmd);
            }
            Message::ThemeCheck => {
                let stamp = theme::stamp();
                if stamp != self.theme_stamp {
                    self.theme_stamp = stamp;
                    theme::load();
                    self.deck.clear();
                }
            }
            _ => {}
        }
        iced::Task::none()
    }

    fn press(&mut self, action: Action) -> iced::Task<Message> {
        match action {
            Action::Mod(m) => {
                if let Some(i) = self.latched.iter().position(|x| *x == m) {
                    self.latched.remove(i);
                } else {
                    self.latched.push(m);
                }
            }
            Action::Caps => self.caps = !self.caps,
            Action::Fn => {
                self.fn_lock = !self.fn_lock;
                self.deck.clear(); // F-row labels change
            }
            Action::Hide => {
                // zenbook-duo-osk drops the panel gap and puts the bottom
                // screen's workspaces back. Run it in its own session so it
                // outlives this process, and drop the pid record first so the
                // gap goes whatever the timing.
                let detached = Command::new("setsid")
                    .arg("-f")
                    .arg(osk_script())
                    .arg("close")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                if detached.is_err() {
                    announce(false);
                } else {
                    forget_pid();
                }
                return iced::exit();
            }
            Action::SwapScreens => {
                let _ = Command::new("hyprctl")
                    .args(["dispatch", "hl.dsp.workspace.swap_monitors({ monitor1 = \"eDP-1\", monitor2 = \"eDP-2\" })"])
                    .spawn();
            }
            Action::Char(sym, plain, shifted) => {
                if self.latched.iter().all(|m| *m == Modifier::Shift) {
                    // Plain typing: send the character itself, so it does not
                    // depend on the active layout's shift level.
                    let shift = self.latched.contains(&Modifier::Shift);
                    let upper = shift ^ (self.caps && plain.is_alphabetic());
                    // Shift is held only where it changes the character.
                    let mods = if upper && shifted != plain { input::MOD_SHIFT } else { 0 };
                    self.send(sym, mods);
                } else {
                    self.combo(sym);
                }
                self.latched.clear();
            }
            Action::Key(sym, fn_sym) => {
                self.combo(if self.fn_lock { fn_sym } else { sym });
                self.latched.clear();
            }
            Action::FRow(media, function) => {
                self.combo(if self.fn_lock { function } else { media });
                self.latched.clear();
            }
            Action::Chord(mods, sym) => self.send(sym, mods),
        }
        iced::Task::none()
    }

    /// Tap `keysym` with the latched modifiers held.
    fn combo(&self, keysym: &'static str) {
        let mods = self.latched.iter().fold(0, |m, l| m | l.mask());
        self.send(keysym, mods);
    }

    fn send(&self, keysym: &'static str, mods: u32) {
        let _ = self.output.send(InputCmd::Key { keysym, mods });
    }

    fn lit(&self, action: Action) -> bool {
        match action {
            Action::Mod(m) => self.latched.contains(&m),
            Action::Caps => self.caps,
            Action::Fn => self.fn_lock,
            _ => false,
        }
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        let theme = iced::time::every(Duration::from_secs(2)).map(|_| Message::ThemeCheck);
        match self.repeat {
            Some(_) => iced::Subscription::batch([iced::time::every(Duration::from_millis(10)).map(Message::Tick), theme]),
            None => theme,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        canvas::Canvas::new(Deck { kb: self }).width(Length::Fill).height(Length::Fill).into()
    }
}

// --- Drawing and input ------------------------------------------------------

struct Deck<'a> {
    kb: &'a Keyboard,
}

const MOUSE: u64 = u64::MAX;
const POINTER_SPEED: f32 = 2.2;
const SCROLL_SPEED: f32 = 0.5;
const TAP_MAX_MS: u128 = 220;
const TAP_SLOP: f32 = 8.0;

#[derive(Default)]
struct Fingers {
    keys: HashMap<u64, usize>,
    pad: HashMap<u64, Point>,
    /// Touchpad gesture in progress: start time, most fingers, moved.
    gesture: Option<(Instant, usize, bool)>,
}

fn deck_rect(bounds: Size, r: Rectangle) -> Rectangle {
    Rectangle {
        x: r.x * bounds.width,
        y: r.y * bounds.height,
        width: r.width * bounds.width,
        height: r.height * bounds.height,
    }
}

impl<'a> Deck<'a> {
    fn key_at(&self, size: Size, p: Point) -> Option<usize> {
        self.kb.keys.iter().position(|k| deck_rect(size, k.rect).contains(p))
    }

    fn on_pad(&self, size: Size, p: Point) -> bool {
        deck_rect(size, frac(PAD.0, PAD.1, PAD.2, PAD.3)).contains(p)
    }

    fn down(&self, st: &mut Fingers, size: Size, id: u64, p: Point) -> Option<canvas::Action<Message>> {
        if let Some(i) = self.key_at(size, p) {
            st.keys.insert(id, i);
            return Some(canvas::Action::publish(Message::KeyDown(i)).and_capture());
        }
        if id != MOUSE && self.on_pad(size, p) {
            let starts = st.pad.is_empty();
            st.pad.insert(id, p);
            let n = st.pad.len();
            st.gesture = match st.gesture {
                Some((t, most, moved)) => Some((t, most.max(n), moved)),
                None => Some((Instant::now(), n, false)),
            };
            if starts {
                return Some(canvas::Action::publish(Message::Input(InputCmd::PadStart)).and_capture());
            }
            return Some(canvas::Action::capture());
        }
        None
    }

    fn moved(&self, st: &mut Fingers, id: u64, p: Point) -> Option<canvas::Action<Message>> {
        let last = *st.pad.get(&id)?;
        let (dx, dy) = (p.x - last.x, p.y - last.y);
        st.pad.insert(id, p);
        if let Some((t, most, moved)) = st.gesture {
            let far = moved || dx.abs() + dy.abs() > TAP_SLOP;
            st.gesture = Some((t, most, far));
        }
        let cmd = if st.pad.len() >= 2 {
            InputCmd::Scroll(dy * SCROLL_SPEED * -1.0)
        } else {
            InputCmd::Motion(dx * POINTER_SPEED, dy * POINTER_SPEED)
        };
        Some(canvas::Action::publish(Message::Input(cmd)).and_capture())
    }

    fn up(&self, st: &mut Fingers, id: u64) -> Option<canvas::Action<Message>> {
        if let Some(i) = st.keys.remove(&id) {
            return Some(canvas::Action::publish(Message::KeyUp(i)).and_capture());
        }
        st.pad.remove(&id)?;
        if !st.pad.is_empty() {
            return Some(canvas::Action::capture());
        }
        let (t, most, moved) = st.gesture.take()?;
        if !moved && t.elapsed().as_millis() <= TAP_MAX_MS {
            let button = if most >= 2 { input::BTN_RIGHT } else { input::BTN_LEFT };
            return Some(canvas::Action::publish(Message::Input(InputCmd::Click(button))).and_capture());
        }
        Some(canvas::Action::capture())
    }
}

impl<'a> canvas::Program<Message> for Deck<'a> {
    type State = Fingers;

    fn update(
        &self,
        st: &mut Fingers,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let size = bounds.size();
        let local = |p: Point| Point::new(p.x - bounds.x, p.y - bounds.y);
        match event {
            Event::Touch(touch::Event::FingerPressed { id, position }) => {
                self.down(st, size, id.0, local(*position))
            }
            Event::Touch(touch::Event::FingerMoved { id, position }) => self.moved(st, id.0, local(*position)),
            Event::Touch(touch::Event::FingerLifted { id, .. } | touch::Event::FingerLost { id, .. }) => {
                self.up(st, id.0)
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = cursor.position_in(bounds)?;
                self.down(st, size, MOUSE, p)
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => self.up(st, MOUSE),
            _ => None,
        }
    }

    fn draw(
        &self,
        _st: &Fingers,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let size = bounds.size();
        let unit_h = size.height * (60.0 / (DECK.3 - DECK.1));
        let kb = self.kb;
        let colors = theme::palette();
        let deck = kb.deck.draw(renderer, size, |frame| {
            // Framed like a window: border colour, then the deck inset by
            // the border width, both with the windows' rounding.
            let (b, r) = (kb.frame.border, kb.frame.rounding);
            frame.fill(&Path::rounded_rectangle(Point::ORIGIN, size, r.into()), colors.border);
            let inner = Size::new(size.width - 2.0 * b, size.height - 2.0 * b);
            frame.fill(&Path::rounded_rectangle(Point::new(b, b), inner, (r - b).max(0.0).into()), colors.deck);
            let pad = deck_rect(size, frac(PAD.0, PAD.1, PAD.2, PAD.3));
            let r = 0.012 * size.height;
            frame.fill(&Path::rounded_rectangle(pad.position(), pad.size(), r.into()), colors.pad);
            for k in &kb.keys {
                draw_key(frame, size, k, kb, unit_h, colors.key);
            }
        });
        // Only pressed and latched keys are drawn per frame.
        let mut frame = Frame::new(renderer, size);
        for (i, k) in kb.keys.iter().enumerate() {
            if kb.held.contains(&i) {
                draw_key(&mut frame, size, k, kb, unit_h, colors.pressed);
            } else if kb.lit(k.action) {
                draw_key(&mut frame, size, k, kb, unit_h, colors.lit);
            }
        }
        vec![deck, frame.into_geometry()]
    }
}

fn draw_key(frame: &mut Frame, size: Size, k: &Placed, kb: &Keyboard, unit_h: f32, fill: Color) {
    let rect = deck_rect(size, k.rect);
    let path = Path::rounded_rectangle(rect.position(), rect.size(), (0.1 * unit_h).into());
    frame.fill(&path, fill);
    // Legends scale with the smaller of the unit key's height and width, so
    // they still fit when the deck is squeezed narrow. In portrait (taller
    // than wide) the keys are tall, so legends get twice that, each shrunk to
    // its key's width, and the function row stacks its labels.
    let unit_w = size.width * (60.0 / (DECK.2 - DECK.0));
    let portrait = size.height > size.width;
    let unit = if portrait { 2.0 * unit_w.min(unit_h) } else { unit_h.min(unit_w) };
    draw_legend(frame, rect, k, kb, unit_h, unit, portrait);
}

fn label(frame: &mut Frame, content: String, at: Point, size: f32, font: Font, align_x: Alignment) {
    label_in(frame, content, at, size, font, align_x, theme::palette().legend);
}

fn label_in(frame: &mut Frame, content: String, at: Point, size: f32, font: Font, align_x: Alignment, color: Color) {
    frame.fill_text(Text {
        content,
        position: at,
        color,
        size: Pixels(size),
        font,
        align_x,
        align_y: Vertical::Center,
        ..Text::default()
    });
}

/// An icon with its ink, not its cell, centred on `at`.
fn draw_icon(frame: &mut Frame, icon: char, at: Point, size: f32) {
    let at = Point::new(at.x - icon_offset(icon) * size, at.y);
    label(frame, icon.to_string(), at, size, ICONS, Alignment::Center);
}

/// `size`, shrunk so `text` spans at most 85% of `width`.
fn fit(size: f32, text: &str, width: f32) -> f32 {
    // Average advance of the UI font, as a fraction of its size.
    size.min(0.85 * width / (0.6 * text.chars().count().max(1) as f32))
}

fn draw_legend(frame: &mut Frame, rect: Rectangle, k: &Placed, kb: &Keyboard, unit_h: f32, unit: f32, portrait: bool) {
    let center = rect.center();
    let main = 0.38 * unit;
    let small = 0.22 * unit;
    // Two lines, one above the other (function row in portrait).
    let upper = Point::new(center.x, rect.y + 0.36 * rect.height);
    let lower = Point::new(center.x, rect.y + 0.74 * rect.height);
    let word = |frame: &mut Frame, s: &str, at: Point, size: f32| {
        label(frame, s.into(), at, fit(size, s, rect.width), Font::DEFAULT, Alignment::Center);
    };
    match k.legend {
        Legend::Main(s) => word(frame, s, center, main),
        Legend::Dual(top, bottom) => {
            let left = rect.x + 0.28 * rect.width;
            label(frame, top.into(), Point::new(left, rect.y + 0.28 * rect.height), small, Font::DEFAULT, Alignment::Center);
            label(frame, bottom.into(), Point::new(center.x, rect.y + 0.62 * rect.height), main, Font::DEFAULT, Alignment::Center);
        }
        Legend::Triple(top, bottom, alt) => {
            // Shifted upper left, plain in the middle, AltGr lower right.
            let left = rect.x + 0.28 * rect.width;
            let right = rect.x + 0.76 * rect.width;
            if !top.is_empty() {
                label(frame, top.into(), Point::new(left, rect.y + 0.28 * rect.height), small, Font::DEFAULT, Alignment::Center);
            }
            let main_at = if top.is_empty() { center } else { Point::new(center.x, rect.y + 0.62 * rect.height) };
            label(frame, bottom.into(), main_at, if top.is_empty() { main } else { main * 0.9 }, Font::DEFAULT, Alignment::Center);
            let alt_at = Point::new(right, rect.y + 0.76 * rect.height);
            label_in(frame, alt.into(), alt_at, small, Font::DEFAULT, Alignment::Center, theme::palette().alt_legend);
        }
        Legend::Word(s) => {
            let s = match k.action {
                Action::FRow(_, f) if kb.fn_lock => f,
                _ => s,
            };
            // "/A  F12", "DEL INS": stacked in portrait.
            match s.split_once(' ') {
                Some((a, b)) if portrait => {
                    let size = 0.3 * rect.height;
                    word(frame, a, upper, size);
                    word(frame, b.trim(), lower, size);
                }
                // F9, F11: as large as the other F-labels.
                _ if portrait && matches!(k.action, Action::FRow(..)) => word(frame, s, center, 0.3 * rect.height),
                _ => word(frame, s, center, small),
            }
        }
        Legend::Icon(_, f) if kb.fn_lock && matches!(k.action, Action::FRow(..)) => {
            word(frame, f, center, small * 1.2);
        }
        Legend::Icon(icon, f) => {
            // Icon-only half-height keys (arrows) fill the key; F-row icons
            // leave room for their F-label, beside it, or above it in
            // portrait. All are capped by the key's width.
            let half = rect.height < 0.7 * unit_h;
            if f.is_empty() {
                // The narrow hide and swap keys get a little more margin than the arrows.
                let w = if matches!(k.action, Action::Hide | Action::SwapScreens) { 0.6 } else { 0.8 };
                let size = if half { (0.95 * rect.height).min(w * rect.width) } else { main };
                draw_icon(frame, icon, center, size);
            } else if portrait {
                let size = (0.42 * rect.height).min(0.6 * rect.width);
                draw_icon(frame, icon, upper, size);
                word(frame, f, lower, 0.3 * rect.height);
            } else {
                let size = if half { (0.62 * rect.height).min(0.36 * rect.width) } else { main };
                let x = rect.x + 0.36 * rect.width;
                draw_icon(frame, icon, Point::new(x, center.y), size);
                let fx = rect.x + 0.74 * rect.width;
                label(frame, f.into(), Point::new(fx, center.y), 0.5 * size, Font::DEFAULT, Alignment::Center);
            }
        }
        Legend::Blank => {}
    }
}

/// Tell the monitor layout whether the keyboard is up: ~/.config/hypr/monitors.lua
/// leaves a gap between the panels while this pid is alive, so the cursor
/// stays off the bottom screen. Closing by `pkill` leaves a dead pid behind,
/// which monitors.lua ignores; zenbook-duo-osk reloads Hyprland itself.
fn announce(up: bool) {
    let file = pid_file();
    if !up {
        forget_pid();
        let _ = Command::new("hyprctl").arg("reload").status();
        return;
    }
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&file, format!("{}\n", std::process::id()));
    // zenbook-duo-osk park applies the gap (reload) and moves workspaces off
    // the bottom screen; doing it from here covers every way of starting.
    let parked = Command::new("setsid")
        .arg("-f")
        .arg(osk_script())
        .arg("park")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if parked.is_err() {
        let _ = Command::new("hyprctl").arg("reload").status();
    }
}

fn osk_script() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config/zenbook/zenbook-duo-osk")
}

fn pid_file() -> std::path::PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"))
        .join("zenbook/osk.pid")
}

fn forget_pid() {
    let _ = std::fs::remove_file(pid_file());
}

fn namespace() -> String {
    String::from("zenbook-duo-keyboard")
}

fn main() -> Result<(), iced_layershell::Error> {
    // Test hook: `zenbook-duo-keyboard --type "text"` types text through the
    // virtual keyboard into the focused window, without showing the deck.
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == "--type" {
        let out = input::spawn("us");
        for c in args[2].chars() {
            let (keysym, mods) = input::char_key(c);
            let _ = out.send(InputCmd::Key { keysym, mods });
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
        return Ok(());
    }
    // GPU rendering (wgpu), with tiny-skia as the fallback iced picks when
    // wgpu cannot start. tiny-skia redraws the whole ~5 Mpx panel on the CPU
    // for every key press, about 280 ms here, which made typing lag.
    announce(true);
    theme::load();
    let gaps = theme::frame().gaps;
    iced_layershell::application(Keyboard::new, namespace, Keyboard::update, Keyboard::view)
        .subscription(Keyboard::subscription)
        // Transparent outside the framed deck (rounded corners).
        .style(|_, _| iced::theme::Style { background_color: Color::TRANSPARENT, text_color: theme::palette().legend })
        .settings(Settings {
            layer_settings: LayerShellSettings {
                anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
                layer: Layer::Top,
                exclusive_zone: -1,
                size: None,
                // The windows' outer gap, so the deck sits like a window.
                margin: (gaps, gaps, gaps, gaps),
                keyboard_interactivity: KeyboardInteractivity::None,
                start_mode: StartMode::TargetScreen(OUTPUT.to_string()),
                ..Default::default()
            },
            ..Default::default()
        })
        .run()
}
