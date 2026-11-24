//! System appearance, selectable palettes, and local appearance preferences.
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};
use std::cell::Cell;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Classic,
    Amber,
    Black,
    Light,
    Ruby,
    Emerald,
    #[default]
    #[serde(other)]
    System,
}

impl Theme {
    pub const ALL: [Theme; 7] = [
        Theme::System,
        Theme::Classic,
        Theme::Amber,
        Theme::Black,
        Theme::Light,
        Theme::Ruby,
        Theme::Emerald,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Theme::System => "Follow system",
            Theme::Classic => "Classic",
            Theme::Amber => "Amber",
            Theme::Black => "Black",
            Theme::Light => "Light",
            Theme::Ruby => "Ruby",
            Theme::Emerald => "Emerald",
        }
    }

    pub fn resolve(self, system: Option<eframe::egui::Theme>) -> Self {
        match (self, system) {
            (Self::System, Some(eframe::egui::Theme::Light)) => Self::Light,
            (Self::System, _) => Self::Black,
            (explicit, _) => explicit,
        }
    }

    pub fn palette(self) -> &'static Palette {
        match self {
            Self::System | Self::Black => &BLACK,
            Self::Classic => &CLASSIC,
            Self::Amber => &AMBER,
            Self::Light => &LIGHT,
            Self::Ruby => &RUBY,
            Self::Emerald => &EMERALD,
        }
    }
}

/// Semantic colors for the app chrome. Video and avatar backplates keep their own colors.
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    /// Primary action, selection and focus.
    pub accent: Color32,
    /// Text and icons drawn on top of `accent`.
    pub on_accent: Color32,
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
    pub divider: Color32,
    pub danger: Color32,
}

const CLASSIC: Palette = Palette {
    dark: true,
    accent: Color32::from_rgb(115, 160, 255),
    on_accent: Color32::from_rgb(5, 11, 21),
    text: Color32::from_rgb(233, 239, 247),
    muted: Color32::from_rgb(154, 170, 203),
    panel: Color32::from_rgb(12, 23, 41),
    canvas: Color32::from_rgb(5, 11, 21),
    card: Color32::from_rgb(17, 30, 53),
    border: Color32::from_rgb(38, 52, 82),
    divider: Color32::from_rgb(38, 52, 82),
    control: Color32::from_rgb(24, 40, 70),
    control_hover: Color32::from_rgb(32, 52, 92),
    control_active: Color32::from_rgb(33, 57, 102),
    hover_stroke: Color32::from_rgb(86, 120, 190),
    success: Color32::from_rgb(45, 212, 128),
    warning: Color32::from_rgb(255, 205, 130),
    error: Color32::from_rgb(255, 165, 170),
    danger: Color32::from_rgb(248, 115, 120),
};

const BLACK: Palette = Palette {
    dark: true,
    accent: Color32::from_rgb(236, 236, 236),
    on_accent: Color32::from_rgb(8, 8, 8),
    text: Color32::from_rgb(245, 245, 245),
    muted: Color32::from_rgb(158, 158, 158),
    panel: Color32::from_rgb(6, 6, 6),
    canvas: Color32::from_rgb(0, 0, 0),
    card: Color32::from_rgb(15, 15, 15),
    border: Color32::from_rgb(46, 46, 46),
    divider: Color32::from_rgb(46, 46, 46),
    control: Color32::from_rgb(24, 24, 24),
    control_hover: Color32::from_rgb(38, 38, 38),
    control_active: Color32::from_rgb(78, 78, 78),
    hover_stroke: Color32::from_rgb(130, 130, 130),
    success: Color32::from_rgb(80, 220, 140),
    warning: Color32::from_rgb(255, 200, 100),
    error: Color32::from_rgb(255, 150, 150),
    danger: Color32::from_rgb(255, 112, 112),
};

const LIGHT: Palette = Palette {
    dark: false,
    accent: Color32::from_rgb(26, 79, 194),
    on_accent: Color32::WHITE,
    text: Color32::from_rgb(18, 27, 46),
    muted: Color32::from_rgb(64, 77, 103),
    panel: Color32::from_rgb(246, 248, 252),
    canvas: Color32::from_rgb(232, 237, 247),
    card: Color32::from_rgb(255, 255, 255),
    border: Color32::from_rgb(206, 214, 230),
    divider: Color32::from_rgb(206, 214, 230),
    control: Color32::from_rgb(233, 238, 248),
    control_hover: Color32::from_rgb(220, 229, 246),
    control_active: Color32::from_rgb(190, 208, 248),
    hover_stroke: Color32::from_rgb(120, 150, 225),
    success: Color32::from_rgb(12, 103, 56),
    warning: Color32::from_rgb(133, 75, 0),
    error: Color32::from_rgb(190, 30, 55),
    danger: Color32::from_rgb(205, 40, 60),
};

const RUBY: Palette = Palette {
    dark: true,
    accent: Color32::from_rgb(255, 110, 120),
    on_accent: Color32::from_rgb(32, 4, 8),
    text: Color32::from_rgb(255, 240, 236),
    muted: Color32::from_rgb(232, 172, 164),
    panel: Color32::from_rgb(40, 8, 12),
    canvas: Color32::from_rgb(22, 4, 6),
    card: Color32::from_rgb(62, 12, 18),
    border: Color32::from_rgb(120, 30, 36),
    divider: Color32::from_rgb(120, 30, 36),
    control: Color32::from_rgb(78, 16, 22),
    control_hover: Color32::from_rgb(108, 22, 30),
    control_active: Color32::from_rgb(120, 26, 34),
    hover_stroke: Color32::from_rgb(255, 214, 10),
    success: Color32::from_rgb(255, 214, 10),
    warning: Color32::from_rgb(255, 214, 10),
    error: Color32::from_rgb(255, 170, 160),
    danger: Color32::from_rgb(255, 120, 110),
};

const EMERALD: Palette = Palette {
    dark: true,
    accent: Color32::from_rgb(255, 205, 20),
    on_accent: Color32::from_rgb(2, 32, 18),
    text: Color32::from_rgb(240, 255, 244),
    muted: Color32::from_rgb(166, 214, 184),
    panel: Color32::from_rgb(4, 40, 26),
    canvas: Color32::from_rgb(2, 24, 16),
    card: Color32::from_rgb(8, 60, 39),
    border: Color32::from_rgb(22, 100, 62),
    divider: Color32::from_rgb(22, 100, 62),
    control: Color32::from_rgb(10, 72, 45),
    control_hover: Color32::from_rgb(14, 100, 62),
    control_active: Color32::from_rgb(14, 95, 61),
    hover_stroke: Color32::from_rgb(61, 220, 110),
    success: Color32::from_rgb(61, 220, 110),
    warning: Color32::from_rgb(255, 205, 20),
    error: Color32::from_rgb(255, 170, 170),
    danger: Color32::from_rgb(255, 125, 125),
};

const AMBER: Palette = Palette {
    dark: true,
    accent: Color32::from_rgb(229, 169, 60),
    on_accent: Color32::from_rgb(18, 16, 11),
    text: Color32::from_rgb(242, 240, 235),
    muted: Color32::from_rgb(166, 163, 155),
    panel: Color32::from_rgb(22, 22, 25),
    canvas: Color32::from_rgb(14, 14, 16),
    card: Color32::from_rgb(30, 30, 35),
    border: Color32::from_rgb(43, 43, 51),
    divider: Color32::from_rgb(35, 35, 42),
    control: Color32::from_rgb(30, 30, 35),
    control_hover: Color32::from_rgb(40, 40, 47),
    control_active: Color32::from_rgb(64, 51, 31),
    hover_stroke: Color32::from_rgb(76, 72, 66),
    success: Color32::from_rgb(92, 184, 122),
    warning: Color32::from_rgb(242, 201, 125),
    error: Color32::from_rgb(255, 154, 158),
    danger: Color32::from_rgb(229, 72, 77),
};

thread_local! {
    // Custom painters run synchronously on the UI thread. The app selects its
    // effective palette before drawing so other app/context state stays separate.
    static ACTIVE: Cell<Theme> = const { Cell::new(Theme::Amber) };
}

pub fn activate(theme: Theme) {
    ACTIVE.set(theme);
}

pub fn palette() -> &'static Palette {
    ACTIVE.get().palette()
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub theme: Theme,
    pub portuguese: Option<bool>,
    #[serde(skip)]
    pub path: Option<std::path::PathBuf>,
}

const MAX_BYTES: u64 = 4096;

impl Appearance {
    pub fn load() -> Self {
        let path = crate::settings::appearance_path();
        let mut appearance = path
            .as_deref()
            .and_then(|path| Self::read(path).ok())
            .unwrap_or_default();
        appearance.path = path;
        appearance
    }

    fn read(path: &std::path::Path) -> std::io::Result<Self> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Appearance file is too large",
            ));
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };
        self.write(path)
    }

    fn write(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let directory = path.parent().expect("appearance directory");
        std::fs::create_dir_all(directory)?;
        let mut file = tempfile::NamedTempFile::new_in(directory)?;
        file.write_all(&serde_json::to_vec(self)?)?;
        file.persist(path)?;
        Ok(())
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
            let p = theme.palette();
            assert!(ratio(p.text, p.panel) >= 7.0, "{theme:?} text on panel");
            assert!(ratio(p.text, p.card) >= 7.0, "{theme:?} text on card");
            assert!(ratio(p.muted, p.panel) >= 4.5, "{theme:?} muted on panel");
            assert!(ratio(p.muted, p.canvas) >= 4.5, "{theme:?} muted on canvas");
            assert!(
                ratio(p.on_accent, p.accent) >= 4.5,
                "{theme:?} label on accent"
            );
            for surface in [
                p.canvas,
                p.panel,
                p.card,
                p.control,
                p.control_hover,
                p.control_active,
            ] {
                assert!(
                    ratio(p.text, surface) >= 4.5,
                    "{theme:?} text on {surface:?}"
                );
            }
            for ink in [p.muted, p.accent, p.success, p.warning, p.error] {
                assert!(ratio(ink, p.panel) >= 4.5, "{theme:?} {ink:?} on panel");
            }
        }
    }

    #[test]
    fn system_follows_light_and_dark_while_overrides_stay_fixed() {
        use eframe::egui::Theme as System;
        assert_eq!(Theme::default().resolve(Some(System::Light)), Theme::Light);
        assert_eq!(Theme::System.resolve(Some(System::Dark)), Theme::Black);
        assert_eq!(Theme::System.resolve(None), Theme::Black);
        assert_eq!(Theme::Amber.resolve(Some(System::Light)), Theme::Amber);
    }

    #[test]
    fn appearance_round_trips_and_falls_back_for_unknown_themes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("appearance.json");
        std::fs::write(&path, br#"{"theme":"unknown","portuguese":false}"#).unwrap();
        let mut appearance = Appearance::read(&path).unwrap();
        assert_eq!(appearance.theme, Theme::System);
        assert_eq!(appearance.portuguese, Some(false));
        appearance.theme = Theme::Emerald;
        appearance.write(&path).unwrap();
        assert_eq!(Appearance::read(&path).unwrap().theme, Theme::Emerald);
        std::fs::write(&path, [b' '; 4097]).unwrap();
        assert!(Appearance::read(&path).is_err());
        std::fs::write(&path, b"malformed").unwrap();
        assert!(Appearance::read(&path).is_err());
    }
}
