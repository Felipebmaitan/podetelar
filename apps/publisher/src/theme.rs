//! Appearance: four selectable palettes and the saved appearance choice.
//!
//! Only the theme and interface language are stored (never servers, names,
//! invitations or credentials). A malformed or missing file means defaults.
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Classic,
    Light,
    Comunista,
    Patriota,
}

impl Theme {
    pub const ALL: [Theme; 4] = [
        Theme::Classic,
        Theme::Light,
        Theme::Comunista,
        Theme::Patriota,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Theme::Classic => "Classic",
            Theme::Light => "Light",
            Theme::Comunista => "Communist",
            Theme::Patriota => "Patriot",
        }
    }

    fn index(self) -> u8 {
        self as u8
    }

    fn from_index(index: u8) -> Theme {
        Theme::ALL
            .get(index as usize)
            .copied()
            .unwrap_or(Theme::Classic)
    }
}

/// Every color the interface draws with. Widgets never hard-code colors.
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    /// Primary action, selection and focus.
    pub accent: Color32,
    /// Text and icons drawn on top of `accent`.
    pub on_accent: Color32,
    /// Secondary highlight (stars, hover rings, badges).
    pub spark: Color32,
    pub text: Color32,
    pub muted: Color32,
    /// Sidebar and header surface.
    pub panel: Color32,
    /// Stage behind the streams.
    pub canvas: Color32,
    /// Raised cards and dialogs.
    pub card: Color32,
    pub border: Color32,
    pub control: Color32,
    pub control_hover: Color32,
    pub control_active: Color32,
    pub hover_stroke: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub danger: Color32,
    pub danger_bg: Color32,
    /// Chips drawn over video.
    pub overlay: Color32,
}

const CLASSIC: Palette = Palette {
    dark: true,
    accent: Color32::from_rgb(79, 134, 247),
    on_accent: Color32::from_rgb(5, 11, 21),
    spark: Color32::from_rgb(255, 207, 36),
    text: Color32::from_rgb(233, 239, 247),
    muted: Color32::from_rgb(154, 170, 203),
    panel: Color32::from_rgb(12, 23, 41),
    canvas: Color32::from_rgb(5, 11, 21),
    card: Color32::from_rgb(17, 30, 53),
    border: Color32::from_rgb(38, 52, 82),
    control: Color32::from_rgb(24, 40, 70),
    control_hover: Color32::from_rgb(32, 52, 92),
    control_active: Color32::from_rgb(38, 84, 190),
    hover_stroke: Color32::from_rgb(86, 120, 190),
    success: Color32::from_rgb(45, 212, 128),
    warning: Color32::from_rgb(255, 205, 130),
    error: Color32::from_rgb(255, 165, 170),
    danger: Color32::from_rgb(248, 115, 120),
    danger_bg: Color32::from_rgb(73, 39, 45),
    overlay: Color32::from_rgba_premultiplied(8, 15, 28, 225),
};

const LIGHT: Palette = Palette {
    dark: false,
    accent: Color32::from_rgb(37, 99, 235),
    on_accent: Color32::WHITE,
    spark: Color32::from_rgb(217, 140, 0),
    text: Color32::from_rgb(18, 27, 46),
    muted: Color32::from_rgb(82, 96, 124),
    panel: Color32::from_rgb(246, 248, 252),
    canvas: Color32::from_rgb(232, 237, 247),
    card: Color32::from_rgb(255, 255, 255),
    border: Color32::from_rgb(206, 214, 230),
    control: Color32::from_rgb(233, 238, 248),
    control_hover: Color32::from_rgb(220, 229, 246),
    control_active: Color32::from_rgb(190, 208, 248),
    hover_stroke: Color32::from_rgb(120, 150, 225),
    success: Color32::from_rgb(18, 128, 74),
    warning: Color32::from_rgb(166, 98, 0),
    error: Color32::from_rgb(190, 30, 55),
    danger: Color32::from_rgb(205, 40, 60),
    danger_bg: Color32::from_rgb(253, 226, 230),
    overlay: Color32::from_rgba_premultiplied(250, 252, 255, 232),
};

const COMUNISTA: Palette = Palette {
    dark: true,
    accent: Color32::from_rgb(224, 32, 40),
    on_accent: Color32::WHITE,
    spark: Color32::from_rgb(255, 214, 10),
    text: Color32::from_rgb(255, 240, 236),
    muted: Color32::from_rgb(232, 172, 164),
    panel: Color32::from_rgb(40, 8, 12),
    canvas: Color32::from_rgb(22, 4, 6),
    card: Color32::from_rgb(62, 12, 18),
    border: Color32::from_rgb(120, 30, 36),
    control: Color32::from_rgb(78, 16, 22),
    control_hover: Color32::from_rgb(108, 22, 30),
    control_active: Color32::from_rgb(170, 22, 30),
    hover_stroke: Color32::from_rgb(255, 214, 10),
    success: Color32::from_rgb(255, 214, 10),
    warning: Color32::from_rgb(255, 214, 10),
    error: Color32::from_rgb(255, 170, 160),
    danger: Color32::from_rgb(255, 120, 110),
    danger_bg: Color32::from_rgb(92, 14, 20),
    overlay: Color32::from_rgba_premultiplied(26, 4, 7, 228),
};

const PATRIOTA: Palette = Palette {
    dark: true,
    accent: Color32::from_rgb(255, 205, 20),
    on_accent: Color32::from_rgb(2, 32, 18),
    spark: Color32::from_rgb(61, 220, 110),
    text: Color32::from_rgb(240, 255, 244),
    muted: Color32::from_rgb(166, 214, 184),
    panel: Color32::from_rgb(4, 40, 26),
    canvas: Color32::from_rgb(2, 24, 16),
    card: Color32::from_rgb(8, 60, 39),
    border: Color32::from_rgb(22, 100, 62),
    control: Color32::from_rgb(10, 72, 45),
    control_hover: Color32::from_rgb(14, 100, 62),
    control_active: Color32::from_rgb(0, 140, 62),
    hover_stroke: Color32::from_rgb(61, 220, 110),
    success: Color32::from_rgb(61, 220, 110),
    warning: Color32::from_rgb(255, 205, 20),
    error: Color32::from_rgb(255, 170, 170),
    danger: Color32::from_rgb(255, 125, 125),
    danger_bg: Color32::from_rgb(88, 30, 36),
    overlay: Color32::from_rgba_premultiplied(2, 26, 17, 228),
};

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Theme {
    Theme::from_index(CURRENT.load(Ordering::Relaxed))
}

pub fn select(theme: Theme) {
    CURRENT.store(theme.index(), Ordering::Relaxed);
}

pub fn palette_of(theme: Theme) -> Palette {
    match theme {
        Theme::Classic => CLASSIC,
        Theme::Light => LIGHT,
        Theme::Comunista => COMUNISTA,
        Theme::Patriota => PATRIOTA,
    }
}

/// The active palette. Cheap enough to call while painting.
pub fn pal() -> Palette {
    palette_of(current())
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct Saved {
    theme: Option<Theme>,
    portuguese: Option<bool>,
}

fn file() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    let root = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from)?;
    #[cfg(not(windows))]
    let root = std::path::PathBuf::from(std::env::var_os("HOME")?)
        .join("Library")
        .join("Application Support");
    root.is_absolute()
        .then(|| root.join("PodeTelar").join("appearance.json"))
}

fn parse(bytes: &[u8]) -> Saved {
    if bytes.len() > 4096 {
        return Saved::default();
    }
    serde_json::from_slice(bytes).unwrap_or_default()
}

/// Apply the saved appearance, if any. Never fails; defaults are used instead.
pub fn load() {
    let Some(saved) = file()
        .and_then(|path| std::fs::read(path).ok())
        .map(|b| parse(&b))
    else {
        return;
    };
    if let Some(theme) = saved.theme {
        select(theme);
    }
    if let Some(portuguese) = saved.portuguese {
        crate::locale::select(portuguese);
    }
}

/// Remember the current theme and language for the next launch.
pub fn save() {
    let Some(path) = file() else { return };
    let saved = Saved {
        theme: Some(current()),
        portuguese: Some(crate::locale::portuguese()),
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec(&saved) {
        let _ = std::fs::write(path, bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_has_readable_text_and_action_contrast() {
        fn luminance(c: Color32) -> f32 {
            let f = |v: u8| {
                let v = v as f32 / 255.0;
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
        }
        fn ratio(a: Color32, b: Color32) -> f32 {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        }
        for theme in Theme::ALL {
            let p = palette_of(theme);
            assert!(ratio(p.text, p.panel) >= 7.0, "{theme:?} text on panel");
            assert!(ratio(p.text, p.card) >= 7.0, "{theme:?} text on card");
            assert!(ratio(p.muted, p.panel) >= 4.5, "{theme:?} muted on panel");
            assert!(ratio(p.muted, p.canvas) >= 4.5, "{theme:?} muted on canvas");
            assert!(
                ratio(p.on_accent, p.accent) >= 4.5,
                "{theme:?} label on accent"
            );
            assert!(ratio(p.text, p.control) >= 4.5, "{theme:?} text on control");
        }
    }

    #[test]
    fn malformed_or_oversized_files_use_defaults() {
        assert!(parse(b"not json").theme.is_none());
        assert!(parse(&vec![b' '; 5000]).theme.is_none());
        assert!(
            parse(br#"{"theme":"patriota","portuguese":false}"#).theme == Some(Theme::Patriota)
        );
    }
}
