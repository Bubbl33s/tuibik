//! Color themes. Each [`Theme`] carries the palette used across the UI: panel
//! borders, titles, accents, the timer phase colors, and the cube sticker
//! scheme. Themes can be cycled at runtime and persisted.

use cube::render::Rgb;
use ratatui::style::Color;

/// A full UI color palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// Stable identifier persisted to the config store.
    pub id: &'static str,
    /// Human-readable name shown in the UI.
    pub name: &'static str,
    /// Panel border color.
    pub border: Color,
    /// Panel title color.
    pub title: Color,
    /// Primary accent (scramble text, markers).
    pub accent: Color,
    /// Selection highlight foreground.
    pub select_fg: Color,
    /// Selection highlight background.
    pub select_bg: Color,
    /// Muted text (labels).
    pub muted: Color,
    /// Normal text.
    pub text: Color,
    /// Graph line color.
    pub graph: Color,
    /// Timer colors by phase: [idle, arming, ready, running, stopped].
    pub timer_idle: Color,
    pub timer_arming: Color,
    pub timer_ready: Color,
    pub timer_running: Color,
    pub timer_stopped: Color,
    /// Cube sticker colors, indexed by face (URFDLB order).
    pub stickers: [Rgb; 6],
}

const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb::new(r, g, b)
}

/// The list of built-in themes.
pub fn all() -> &'static [Theme] {
    THEMES
}

/// Look up a theme by id, falling back to the first theme.
pub fn by_id(id: &str) -> &'static Theme {
    THEMES.iter().find(|t| t.id == id).unwrap_or(&THEMES[0])
}

/// The index of a theme id in the list (for cycling).
pub fn index_of(id: &str) -> usize {
    THEMES.iter().position(|t| t.id == id).unwrap_or(0)
}

/// The theme after `id` in the cycle.
pub fn next(id: &str) -> &'static Theme {
    let i = index_of(id);
    &THEMES[(i + 1) % THEMES.len()]
}

/// The theme before `id` in the cycle.
pub fn prev(id: &str) -> &'static Theme {
    let i = index_of(id);
    &THEMES[(i + THEMES.len() - 1) % THEMES.len()]
}

/// Standard WCA sticker colors (used by most themes).
const WCA: [Rgb; 6] = [
    rgb(0xEE, 0xEE, 0xEE), // U white
    rgb(0xD1, 0x1D, 0x1D), // R red
    rgb(0x1F, 0xB8, 0x3A), // F green
    rgb(0xF2, 0xD1, 0x1B), // D yellow
    rgb(0xF2, 0x8C, 0x1B), // L orange
    rgb(0x1B, 0x54, 0xF2), // B blue
];

/// Pastel sticker set for softer themes.
const PASTEL: [Rgb; 6] = [
    rgb(0xF0, 0xF0, 0xF0), // U
    rgb(0xE8, 0x6A, 0x6A), // R
    rgb(0x7F, 0xC9, 0x8A), // F
    rgb(0xF2, 0xD9, 0x8A), // D
    rgb(0xF2, 0xB0, 0x7A), // L
    rgb(0x7A, 0x9C, 0xF2), // B
];

static THEMES: &[Theme] = &[
    // Nord — cool, modern blue-gray.
    Theme {
        id: "nord",
        name: "Nord",
        border: Color::Rgb(0x4C, 0x56, 0x6A),
        title: Color::Rgb(0x88, 0xC0, 0xD0),
        accent: Color::Rgb(0x8F, 0xBC, 0xBB),
        select_fg: Color::Rgb(0x2E, 0x34, 0x40),
        select_bg: Color::Rgb(0x88, 0xC0, 0xD0),
        muted: Color::Rgb(0x61, 0x6E, 0x88),
        text: Color::Rgb(0xEC, 0xEF, 0xF4),
        graph: Color::Rgb(0xA3, 0xBE, 0x8C),
        timer_idle: Color::Rgb(0xD8, 0xDE, 0xE9),
        timer_arming: Color::Rgb(0xBF, 0x61, 0x6A),
        timer_ready: Color::Rgb(0xA3, 0xBE, 0x8C),
        timer_running: Color::Rgb(0xEB, 0xCB, 0x8B),
        timer_stopped: Color::Rgb(0x88, 0xC0, 0xD0),
        stickers: WCA,
    },
    // Dracula — vibrant purple/pink dark theme.
    Theme {
        id: "dracula",
        name: "Dracula",
        border: Color::Rgb(0x44, 0x47, 0x5A),
        title: Color::Rgb(0xBD, 0x93, 0xF9),
        accent: Color::Rgb(0xFF, 0x79, 0xC6),
        select_fg: Color::Rgb(0x28, 0x2A, 0x36),
        select_bg: Color::Rgb(0xBD, 0x93, 0xF9),
        muted: Color::Rgb(0x62, 0x72, 0xA4),
        text: Color::Rgb(0xF8, 0xF8, 0xF2),
        graph: Color::Rgb(0x50, 0xFA, 0x7B),
        timer_idle: Color::Rgb(0xF8, 0xF8, 0xF2),
        timer_arming: Color::Rgb(0xFF, 0x55, 0x55),
        timer_ready: Color::Rgb(0x50, 0xFA, 0x7B),
        timer_running: Color::Rgb(0xF1, 0xFA, 0x8C),
        timer_stopped: Color::Rgb(0x8B, 0xE9, 0xFD),
        stickers: WCA,
    },
    // Gruvbox — warm retro palette.
    Theme {
        id: "gruvbox",
        name: "Gruvbox",
        border: Color::Rgb(0x50, 0x49, 0x45),
        title: Color::Rgb(0xFA, 0xBD, 0x2F),
        accent: Color::Rgb(0x83, 0xA5, 0x98),
        select_fg: Color::Rgb(0x28, 0x28, 0x28),
        select_bg: Color::Rgb(0xFA, 0xBD, 0x2F),
        muted: Color::Rgb(0x92, 0x83, 0x74),
        text: Color::Rgb(0xEB, 0xDB, 0xB2),
        graph: Color::Rgb(0xB8, 0xBB, 0x26),
        timer_idle: Color::Rgb(0xEB, 0xDB, 0xB2),
        timer_arming: Color::Rgb(0xFB, 0x49, 0x34),
        timer_ready: Color::Rgb(0xB8, 0xBB, 0x26),
        timer_running: Color::Rgb(0xFA, 0xBD, 0x2F),
        timer_stopped: Color::Rgb(0x83, 0xA5, 0x98),
        stickers: WCA,
    },
    // Mono Pastel — a soft, minimal look with pastel stickers.
    Theme {
        id: "pastel",
        name: "Pastel",
        border: Color::Rgb(0x5A, 0x5A, 0x6E),
        title: Color::Rgb(0xC7, 0xB8, 0xEA),
        accent: Color::Rgb(0x9C, 0xD1, 0xBB),
        select_fg: Color::Rgb(0x2A, 0x2A, 0x33),
        select_bg: Color::Rgb(0xC7, 0xB8, 0xEA),
        muted: Color::Rgb(0x77, 0x77, 0x88),
        text: Color::Rgb(0xE8, 0xE8, 0xF0),
        graph: Color::Rgb(0x9C, 0xD1, 0xBB),
        timer_idle: Color::Rgb(0xE8, 0xE8, 0xF0),
        timer_arming: Color::Rgb(0xE8, 0x8A, 0x8A),
        timer_ready: Color::Rgb(0x9C, 0xD1, 0xBB),
        timer_running: Color::Rgb(0xF2, 0xD9, 0x8A),
        timer_stopped: Color::Rgb(0xC7, 0xB8, 0xEA),
        stickers: PASTEL,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_multiple_themes() {
        assert!(all().len() >= 3);
    }

    #[test]
    fn by_id_falls_back() {
        assert_eq!(by_id("nonexistent").id, THEMES[0].id);
        assert_eq!(by_id("dracula").id, "dracula");
    }

    #[test]
    fn next_cycles_and_wraps() {
        let first = THEMES[0].id;
        let last = THEMES[THEMES.len() - 1].id;
        assert_eq!(next(last).id, first, "cycling past the last wraps to first");
        assert_ne!(next(first).id, first);
    }

    #[test]
    fn prev_is_inverse_of_next() {
        for t in all() {
            assert_eq!(prev(next(t.id).id).id, t.id);
        }
    }

    #[test]
    fn every_theme_has_six_stickers() {
        for t in all() {
            assert_eq!(t.stickers.len(), 6);
        }
    }
}
