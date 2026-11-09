// SPDX-FileCopyrightText: 2026 Daniel
// SPDX-License-Identifier: GPL-3.0-or-later

//! System preferences used by media queries.
//!
//! Detection is intentionally best-effort and only runs on load/reload. The
//! wallpaper has no input, so `hover`/`pointer` default to `none`; display
//! mode defaults to `fullscreen` because the surface is a layer shell.

use std::process::Command;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorScheme {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Hover {
    #[default]
    None,
    Hover,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pointer {
    #[default]
    None,
    Coarse,
    Fine,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DisplayMode {
    #[default]
    Fullscreen,
    Browser,
    MinimalUi,
    Standalone,
    PictureInPicture,
    Borderless,
    WindowControlsOverlay,
    Tabbed,
}

impl ColorScheme {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.trim().to_ascii_lowercase().as_str() {
            "light" | "default" | "prefer-light" => Self::Light,
            "dark" | "prefer-dark" => Self::Dark,
            _ => return None,
        })
    }
}

impl Hover {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.trim().to_ascii_lowercase().as_str() {
            "none" => Self::None,
            "hover" => Self::Hover,
            _ => return None,
        })
    }
}

impl Pointer {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.trim().to_ascii_lowercase().as_str() {
            "none" => Self::None,
            "coarse" => Self::Coarse,
            "fine" => Self::Fine,
            _ => return None,
        })
    }
}

impl DisplayMode {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.trim().to_ascii_lowercase().as_str() {
            "fullscreen" => Self::Fullscreen,
            "browser" => Self::Browser,
            "minimal-ui" => Self::MinimalUi,
            "standalone" => Self::Standalone,
            "picture-in-picture" => Self::PictureInPicture,
            "borderless" => Self::Borderless,
            "window-controls-overlay" => Self::WindowControlsOverlay,
            "tabbed" => Self::Tabbed,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub color_scheme: ColorScheme,
    pub reduced_motion: bool,
    pub hover: Hover,
    pub pointer: Pointer,
    pub any_hover: Hover,
    pub any_pointer: Pointer,
    pub display_mode: DisplayMode,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            color_scheme: ColorScheme::Light,
            reduced_motion: false,
            hover: Hover::None,
            pointer: Pointer::None,
            any_hover: Hover::None,
            any_pointer: Pointer::None,
            display_mode: DisplayMode::Fullscreen,
        }
    }
}

impl Preferences {
    /// Detect the desktop preferences. Environment overrides win over the
    /// desktop-specific probes, which keeps tests and non-GNOME sessions
    /// predictable.
    pub fn detect() -> Self {
        let mut preferences = Self::default();

        if let Some(value) =
            env("WIDDGYY_COLOR_SCHEME").and_then(|value| ColorScheme::parse(&value))
        {
            preferences.color_scheme = value;
        } else if let Some(scheme) = detect_color_scheme() {
            preferences.color_scheme = scheme;
        }

        if let Some(value) = env("WIDDGYY_REDUCED_MOTION") {
            preferences.reduced_motion = parse_bool(&value);
        } else if let Some(enabled) = gsettings("org.gnome.desktop.interface", "enable-animations")
            .and_then(|value| parse_bool_opt(&value))
        {
            preferences.reduced_motion = !enabled;
        } else if let Some(factor) = kde("KDE", "AnimationDurationFactor") {
            preferences.reduced_motion =
                factor.trim().parse::<f32>().is_ok_and(|value| value <= 0.0);
        }

        if let Some(value) = env("WIDDGYY_HOVER").and_then(|value| Hover::parse(&value)) {
            preferences.hover = value;
        }
        if let Some(value) = env("WIDDGYY_POINTER").and_then(|value| Pointer::parse(&value)) {
            preferences.pointer = value;
        }
        preferences.any_hover = env("WIDDGYY_ANY_HOVER")
            .and_then(|value| Hover::parse(&value))
            .unwrap_or(preferences.hover);
        preferences.any_pointer = env("WIDDGYY_ANY_POINTER")
            .and_then(|value| Pointer::parse(&value))
            .unwrap_or(preferences.pointer);
        if let Some(value) =
            env("WIDDGYY_DISPLAY_MODE").and_then(|value| DisplayMode::parse(&value))
        {
            preferences.display_mode = value;
        }

        preferences
    }
}

fn detect_color_scheme() -> Option<ColorScheme> {
    gsettings("org.gnome.desktop.interface", "color-scheme")
        .and_then(|value| ColorScheme::parse(&value))
        .or_else(|| {
            gsettings("org.gnome.desktop.interface", "gtk-theme").map(|value| theme_scheme(&value))
        })
        .or_else(|| kde("General", "ColorScheme").map(|value| theme_scheme(&value)))
        .or_else(|| env("GTK_THEME").map(|value| theme_scheme(&value)))
}

fn theme_scheme(value: &str) -> ColorScheme {
    if value.to_ascii_lowercase().contains("dark") {
        ColorScheme::Dark
    } else {
        ColorScheme::Light
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn gsettings(schema: &str, key: &str) -> Option<String> {
    let output = Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string(),
    )
}

fn kde(group: &str, key: &str) -> Option<String> {
    for executable in ["kreadconfig6", "kreadconfig5"] {
        let Some(output) = Command::new(executable)
            .args(["--file", "kdeglobals", "--group", group, "--key", key])
            .output()
            .ok()
        else {
            continue;
        };
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "reduce"
    )
}

fn parse_bool_opt(value: &str) -> Option<bool> {
    Some(match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_css_keywords() {
        assert_eq!(ColorScheme::parse("prefer-dark"), Some(ColorScheme::Dark));
        assert_eq!(ColorScheme::parse("light"), Some(ColorScheme::Light));
        assert_eq!(Hover::parse("hover"), Some(Hover::Hover));
        assert_eq!(Pointer::parse("fine"), Some(Pointer::Fine));
        assert_eq!(
            DisplayMode::parse("picture-in-picture"),
            Some(DisplayMode::PictureInPicture)
        );
        assert_eq!(DisplayMode::parse("unknown"), None);
    }

    #[test]
    fn defaults_are_display_only() {
        let preferences = Preferences::default();
        assert_eq!(preferences.color_scheme, ColorScheme::Light);
        assert!(!preferences.reduced_motion);
        assert_eq!(preferences.hover, Hover::None);
        assert_eq!(preferences.pointer, Pointer::None);
        assert_eq!(preferences.display_mode, DisplayMode::Fullscreen);
    }
}
