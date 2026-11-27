use gpui_kit::{
    App, Rgba, WindowAppearance,
    component::{Theme, theme::ThemeMode},
    px, rgb, rgba,
};
use std::path::PathBuf;

/// User-selected appearance preference (`System` follows the window appearance).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemePref {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePref {
    pub fn parse(s: &str) -> Self {
        match s.trim() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub fn is_dark(self, appearance: WindowAppearance) -> bool {
        match self {
            Self::Light => false,
            Self::Dark => true,
            Self::System => {
                matches!(
                    appearance,
                    WindowAppearance::Dark | WindowAppearance::VibrantDark
                )
            }
        }
    }
}

/// Semantic colors shared by the workspace, dialogs and kit controls.
#[derive(Clone, Copy)]
pub struct Palette {
    pub chrome: Rgba,
    pub surface: Rgba,
    pub surface_elevated: Rgba,
    pub modal_surface: Rgba,
    pub modal_chrome: Rgba,
    pub soft: Rgba,
    pub table_header: Rgba,
    pub row_hover: Rgba,
    pub nav_hover: Rgba,
    pub selected: Rgba,
    pub selected_hover: Rgba,
    pub backdrop: Rgba,
    pub border: Rgba,
    pub border_subtle: Rgba,
    pub accent_border: Rgba,
    pub ink: Rgba,
    pub ink_secondary: Rgba,
    pub muted: Rgba,
    pub primary: Rgba,
    pub action: Rgba,
    pub action_hover: Rgba,
    pub on_action: Rgba,
    pub accent_soft: Rgba,
    pub success: Rgba,
    pub success_soft: Rgba,
    pub warning: Rgba,
    pub danger: Rgba,
    pub danger_soft: Rgba,
    pub danger_border: Rgba,
    pub progress_track: Rgba,
    pub segment_divider: Rgba,
}

pub const BAR_HEIGHT: f32 = 46.;
pub const SIDEBAR_WIDTH: f32 = 208.;
pub const RAIL_WIDTH: f32 = 54.;
pub const PANE_RADIUS: f32 = 12.;
pub const CONTROL_RADIUS: f32 = 6.;

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            chrome: rgb(0x121213),
            surface: rgb(0x19191b),
            surface_elevated: rgb(0x303032),
            modal_surface: rgb(0x1e1e20),
            modal_chrome: rgb(0x222224),
            soft: rgb(0x262628),
            table_header: rgb(0x19191b),
            row_hover: rgb(0x242426),
            nav_hover: rgb(0x29292b),
            selected: rgb(0x34261f),
            selected_hover: rgb(0x403027),
            backdrop: rgba(0x00000092),
            border: rgb(0x353538),
            border_subtle: rgb(0x2a2a2c),
            accent_border: rgb(0x8e573b),
            ink: rgb(0xf0eff0),
            ink_secondary: rgb(0xc6c4c6),
            muted: rgb(0xaaa8aa),
            primary: rgb(0xf1a47a),
            action: rgb(0xf1a47a),
            action_hover: rgb(0xffb990),
            on_action: rgb(0x29150b),
            accent_soft: rgb(0x36271f),
            success: rgb(0x86ce9c),
            success_soft: rgb(0x203029),
            warning: rgb(0xe7be7b),
            danger: rgb(0xff9da8),
            danger_soft: rgb(0x3a252a),
            danger_border: rgb(0x6d3842),
            progress_track: rgb(0x373638),
            segment_divider: rgb(0xff9da8),
        }
    } else {
        Palette {
            chrome: rgb(0xefefef),
            surface: rgb(0xfbfbfb),
            surface_elevated: rgb(0xffffff),
            modal_surface: rgb(0xfbfbfb),
            modal_chrome: rgb(0xf5f5f5),
            soft: rgb(0xededed),
            table_header: rgb(0xfbfbfb),
            row_hover: rgb(0xf1f1f1),
            nav_hover: rgb(0xe2e2e2),
            selected: rgb(0xf4e3d9),
            selected_hover: rgb(0xf0d9cb),
            backdrop: rgba(0x16131359),
            border: rgb(0xd5d4d4),
            border_subtle: rgb(0xe7e6e6),
            accent_border: rgb(0xd59d80),
            ink: rgb(0x232225),
            ink_secondary: rgb(0x4a484b),
            muted: rgb(0x69666a),
            primary: rgb(0xa04422),
            action: rgb(0xa04422),
            action_hover: rgb(0x843518),
            on_action: rgb(0xffffff),
            accent_soft: rgb(0xf6e7df),
            success: rgb(0x286b43),
            success_soft: rgb(0xe5f0e8),
            warning: rgb(0x865b20),
            danger: rgb(0xa32e40),
            danger_soft: rgb(0xf9e7ea),
            danger_border: rgb(0xe4b5bd),
            progress_track: rgb(0xe7e4e5),
            segment_divider: rgb(0xa32e40),
        }
    }
}

/// Returns an icon color and matching low-contrast surface for a file category.
pub fn category_tint(category: &str, dark: bool) -> (Rgba, Rgba) {
    let (fg, bg) = match (category, dark) {
        ("Torrents", false) => (0x654cac, 0xeee7f7),
        ("Torrents", true) => (0xbba5ed, 0x2e263e),
        ("Video", false) => (0xa44463, 0xf7e7ed),
        ("Video", true) => (0xeb9ab6, 0x3a2933),
        ("Music", false) => (0x167361, 0xe4f1ec),
        ("Music", true) => (0x81cfb9, 0x233632),
        ("Compressed", false) => (0x8a5b20, 0xf7eee0),
        ("Compressed", true) => (0xe2ba79, 0x3d3022),
        ("Documents", false) => (0x325e9b, 0xe8eef8),
        ("Documents", true) => (0x99b9ec, 0x263246),
        ("Programs", false) => (0x476c35, 0xe9f0e5),
        ("Programs", true) => (0xa4cb90, 0x2c3728),
        (_, false) => (0x635e69, 0xeeecef),
        (_, true) => (0xc0bbc6, 0x2d2a30),
    };
    (rgb(fg), rgb(bg))
}

fn theme_pref_path() -> Option<PathBuf> {
    ndm_ipc::data_dir().map(|d| d.join("ui-theme.txt"))
}

/// Loads the persisted theme preference, defaulting to following the system appearance.
pub fn load_theme_pref() -> ThemePref {
    theme_pref_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| ThemePref::parse(&s))
        .unwrap_or_default()
}

/// Persists the selected theme preference to the application data directory.
pub fn save_theme_pref(pref: ThemePref) {
    if let Some(path) = theme_pref_path() {
        let _ = std::fs::write(path, pref.as_str());
    }
}

/// Synchronizes `gpui_kit::component::Theme` with the active Light or Dark palette.
pub fn apply(dark: bool, cx: &mut App) {
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        None,
        cx,
    );
    let p = palette(dark);
    Theme::update(cx, |t| {
        t.font_family = ".SystemUIFont".into();
        t.font_size = px(13.);
        t.radius = px(CONTROL_RADIUS);
        t.radius_lg = px(PANE_RADIUS);
        t.shadow = false;
        t.focus_ring = true;
        t.background = p.modal_surface.into();
        t.foreground = p.ink.into();
        t.border = p.border.into();
        t.input = p.border.into();
        t.muted = p.soft.into();
        t.muted_foreground = p.muted.into();
        t.secondary = p.soft.into();
        t.secondary_foreground = p.ink.into();
        t.secondary_hover = p.nav_hover.into();
        t.button = p.surface_elevated.into();
        t.button_hover = p.soft.into();
        t.button_active = p.nav_hover.into();
        t.button_foreground = p.ink.into();
        t.primary = p.action.into();
        t.primary_foreground = p.on_action.into();
        t.ring = p.primary.into();
        t.caret = p.primary.into();
        t.button_primary = p.action.into();
        t.button_primary_hover = p.action_hover.into();
        t.button_primary_active = p.action_hover.into();
        t.button_primary_foreground = p.on_action.into();
        t.button_danger = rgb(0xa32e40).into();
        t.button_danger_hover = rgb(0x8b2435).into();
        t.button_danger_active = rgb(0x8b2435).into();
        t.button_danger_foreground = rgb(0xffffff).into();
        t.accent = p.accent_soft.into();
        t.accent_foreground = p.primary.into();
        t.selection = p.selected.into();
        t.switch = p.progress_track.into();
        t.switch_thumb = rgb(0xffffff).into();
        t.popover = p.surface_elevated.into();
        t.popover_foreground = p.ink.into();
        t.scrollbar = p.modal_surface.into();
        t.scrollbar_thumb = p.border.into();
        t.scrollbar_thumb_hover = p.muted.into();
    });
}

/// Installs an initial theme before the first window opens.
pub fn install(cx: &mut App) {
    apply(load_theme_pref().is_dark(WindowAppearance::Light), cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_pref_roundtrip() {
        assert_eq!(ThemePref::parse("system"), ThemePref::System);
        assert_eq!(ThemePref::parse("light"), ThemePref::Light);
        assert_eq!(ThemePref::parse("dark"), ThemePref::Dark);
        assert_eq!(ThemePref::parse("unknown"), ThemePref::System);

        assert_eq!(ThemePref::System.as_str(), "system");
        assert_eq!(ThemePref::Light.as_str(), "light");
        assert_eq!(ThemePref::Dark.as_str(), "dark");
    }

    #[test]
    fn theme_pref_resolves_system_appearance() {
        assert!(!ThemePref::System.is_dark(WindowAppearance::Light));
        assert!(!ThemePref::System.is_dark(WindowAppearance::VibrantLight));
        assert!(ThemePref::System.is_dark(WindowAppearance::Dark));
        assert!(ThemePref::System.is_dark(WindowAppearance::VibrantDark));

        assert!(!ThemePref::Light.is_dark(WindowAppearance::Dark));
        assert!(!ThemePref::Light.is_dark(WindowAppearance::VibrantDark));
        assert!(ThemePref::Dark.is_dark(WindowAppearance::Light));
        assert!(ThemePref::Dark.is_dark(WindowAppearance::VibrantLight));
    }
}
