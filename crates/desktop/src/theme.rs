use gpui_kit::{
    App, Rgba, WindowAppearance,
    component::{Theme, theme::ThemeMode},
    px, rgb, rgba,
};
use std::path::PathBuf;

/// User-selected appearance preference (`System` follows macOS Light/Dark mode).
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

    pub fn next(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
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

/// Cohesive semantic color palette for the native GPUI desktop workspace.
#[derive(Clone, Copy)]
pub struct Palette {
    pub chrome: Rgba,
    pub sidebar: Rgba,
    pub toolbar: Rgba,
    pub surface: Rgba,
    pub surface_elevated: Rgba,
    pub modal_surface: Rgba,
    pub modal_chrome: Rgba,
    pub glass_card: Rgba,
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
    pub primary_bright: Rgba,
    pub primary_hover: Rgba,
    pub accent_soft: Rgba,
    pub success: Rgba,
    pub success_soft: Rgba,
    pub success_border: Rgba,
    pub warning: Rgba,
    pub warning_soft: Rgba,
    pub danger: Rgba,
    pub danger_soft: Rgba,
    pub danger_border: Rgba,
    pub notice_bg: Rgba,
    pub notice_border: Rgba,
    pub notice_text: Rgba,
    pub progress_track: Rgba,
    pub segment_divider: Rgba,
}

/// Returns the calibrated frosted Light or Dark semantic palette.
pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            chrome: rgba(0x0b1120b8),
            sidebar: rgba(0x0d1424b0),
            toolbar: rgba(0x0f172ad2),
            surface: rgba(0x0f172ae6),
            surface_elevated: rgb(0x1e293b),
            modal_surface: rgb(0x0f172a),
            modal_chrome: rgb(0x0b1120),
            glass_card: rgba(0x1e293bba),
            soft: rgb(0x162033),
            table_header: rgba(0x111a2ec4),
            row_hover: rgba(0xf8fafc0c),
            nav_hover: rgba(0xf8fafc12),
            selected: rgba(0x38bdf81c),
            selected_hover: rgba(0x38bdf828),
            backdrop: rgba(0x00000088),
            border: rgba(0xf8fafc1c),
            border_subtle: rgba(0xf8fafc0f),
            accent_border: rgb(0x075985),
            ink: rgb(0xf1f5f9),
            ink_secondary: rgb(0xcbd5e1),
            muted: rgb(0x94a3b8),
            primary: rgb(0x38bdf8),
            primary_bright: rgb(0x7dd3fc),
            primary_hover: rgb(0x0ea5e9),
            accent_soft: rgb(0x0c2d48),
            success: rgb(0x34d399),
            success_soft: rgb(0x062d22),
            success_border: rgb(0x065f46),
            warning: rgb(0xfbbf24),
            warning_soft: rgb(0x2d1f06),
            danger: rgb(0xfb7185),
            danger_soft: rgb(0x2d0f18),
            danger_border: rgb(0x881337),
            notice_bg: rgb(0x1e293b),
            notice_border: rgb(0x334155),
            notice_text: rgb(0xf1f5f9),
            progress_track: rgb(0x1e293b),
            segment_divider: rgb(0xf43f5e),
        }
    } else {
        Palette {
            chrome: rgba(0xf8fafcb8),
            sidebar: rgba(0xf1f5f9b0),
            toolbar: rgba(0xffffffd4),
            surface: rgba(0xffffffeb),
            surface_elevated: rgb(0xffffff),
            modal_surface: rgb(0xffffff),
            modal_chrome: rgb(0xf8fafc),
            glass_card: rgba(0xffffffcc),
            soft: rgb(0xf1f5f9),
            table_header: rgba(0xf8fafcc8),
            row_hover: rgba(0x0f172a0a),
            nav_hover: rgba(0x0f172a0f),
            selected: rgba(0x0284c714),
            selected_hover: rgba(0x0284c720),
            backdrop: rgba(0x02061766),
            border: rgba(0x0f172a1c),
            border_subtle: rgba(0x0f172a10),
            accent_border: rgb(0xbae6fd),
            ink: rgb(0x0f172a),
            ink_secondary: rgb(0x334155),
            muted: rgb(0x64748b),
            primary: rgb(0x0284c7),
            primary_bright: rgb(0x0ea5e9),
            primary_hover: rgb(0x0369a1),
            accent_soft: rgb(0xe0f2fe),
            success: rgb(0x059669),
            success_soft: rgb(0xecfdf5),
            success_border: rgb(0xa7f3d0),
            warning: rgb(0xd97706),
            warning_soft: rgb(0xfffbeb),
            danger: rgb(0xe11d48),
            danger_soft: rgb(0xfff1f2),
            danger_border: rgb(0xfecdd3),
            notice_bg: rgb(0xffffff),
            notice_border: rgb(0xe2e8f0),
            notice_text: rgb(0x0f172a),
            progress_track: rgb(0xe2e8f0),
            segment_divider: rgb(0xfb7185),
        }
    }
}

/// Returns `(icon_color, tile_background)` for a download category.
pub fn category_tint(category: &str, dark: bool) -> (Rgba, Rgba) {
    match (category, dark) {
        ("Torrents", false) => (rgb(0x6366f1), rgb(0xeef2ff)),
        ("Torrents", true) => (rgb(0x818cf8), rgb(0x1e1b4b)),
        ("Video", false) => (rgb(0xe11d48), rgb(0xfff1f2)),
        ("Video", true) => (rgb(0xfb7185), rgb(0x2d0f18)),
        ("Music", false) => (rgb(0xd97706), rgb(0xfffbeb)),
        ("Music", true) => (rgb(0xfbbf24), rgb(0x2d1f06)),
        ("Compressed", false) => (rgb(0x0d9488), rgb(0xf0fdfa)),
        ("Compressed", true) => (rgb(0x2dd4bf), rgb(0x082f2c)),
        ("Documents", false) => (rgb(0x0284c7), rgb(0xe0f2fe)),
        ("Documents", true) => (rgb(0x38bdf8), rgb(0x0c2d48)),
        ("Programs", false) => (rgb(0x059669), rgb(0xecfdf5)),
        ("Programs", true) => (rgb(0x34d399), rgb(0x062d22)),
        (_, false) => (rgb(0x475569), rgb(0xf1f5f9)),
        (_, true) => (rgb(0x94a3b8), rgb(0x1e293b)),
    }
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
    let btn_primary = if dark { rgb(0x0284c7) } else { p.primary };
    let btn_primary_hover = if dark { rgb(0x0369a1) } else { p.primary_hover };
    let btn_danger = rgb(0xe11d48);
    let btn_danger_hover = rgb(0xbe123c);
    let input_border = if dark { rgb(0x334155) } else { rgb(0xd4dce6) };
    Theme::update(cx, |t| {
        t.font_family = ".SystemUIFont".into();
        t.font_size = px(13.);
        t.radius = px(7.);
        t.radius_lg = px(14.);
        t.shadow = false;
        t.focus_ring = true;
        t.background = p.modal_surface.into();
        t.foreground = p.ink.into();
        t.border = p.border.into();
        t.input = input_border.into();
        t.muted = p.soft.into();
        t.muted_foreground = p.muted.into();
        t.secondary = p.soft.into();
        t.secondary_foreground = p.ink.into();
        t.secondary_hover = p.nav_hover.into();
        t.button = p.surface_elevated.into();
        t.button_hover = p.soft.into();
        t.button_active = p.nav_hover.into();
        t.button_foreground = p.ink.into();
        t.primary = p.primary.into();
        t.primary_foreground = rgb(0xffffff).into();
        t.ring = p.primary.into();
        t.caret = p.primary.into();
        t.button_primary = btn_primary.into();
        t.button_primary_hover = btn_primary_hover.into();
        t.button_primary_active = btn_primary_hover.into();
        t.button_primary_foreground = rgb(0xffffff).into();
        t.button_danger = btn_danger.into();
        t.button_danger_hover = btn_danger_hover.into();
        t.button_danger_active = btn_danger_hover.into();
        t.button_danger_foreground = rgb(0xffffff).into();
        t.accent = p.accent_soft.into();
        t.accent_foreground = p.primary.into();
        t.selection = if dark {
            rgba(0x38bdf833).into()
        } else {
            rgba(0x0284c733).into()
        };
        t.switch = if dark {
            rgb(0x334155).into()
        } else {
            rgb(0xcbd5e1).into()
        };
        t.switch_thumb = rgb(0xffffff).into();
        t.popover = p.surface_elevated.into();
        t.popover_foreground = p.ink.into();
        t.scrollbar = p.modal_surface.into();
        t.scrollbar_thumb = if dark {
            rgba(0x47556999).into()
        } else {
            rgba(0x94a3b877).into()
        };
        t.scrollbar_thumb_hover = if dark {
            rgba(0x64748bcc).into()
        } else {
            rgba(0x64748baa).into()
        };
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
    fn theme_pref_roundtrip_and_cycle() {
        assert_eq!(ThemePref::parse("system"), ThemePref::System);
        assert_eq!(ThemePref::parse("light"), ThemePref::Light);
        assert_eq!(ThemePref::parse("dark"), ThemePref::Dark);
        assert_eq!(ThemePref::parse("unknown"), ThemePref::System);

        assert_eq!(ThemePref::System.as_str(), "system");
        assert_eq!(ThemePref::Light.as_str(), "light");
        assert_eq!(ThemePref::Dark.as_str(), "dark");

        assert_eq!(ThemePref::System.next(), ThemePref::Light);
        assert_eq!(ThemePref::Light.next(), ThemePref::Dark);
        assert_eq!(ThemePref::Dark.next(), ThemePref::System);
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
