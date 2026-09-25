//! Typography and a blue mail-client accent over gpui-component's neutral theme.
//!
//! `Theme::change` reloads the stock colors, so every mode switch goes
//! through [`apply`] to re-tint them.

use gpui_kit::{
    App, Hsla, Window,
    component::{Theme, ThemeMode, ThemeTokens},
    px, rgb,
};

pub(crate) fn configure(cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.font_size = px(14.);
    theme.radius = px(8.);
    theme.radius_lg = px(10.);
    Theme::sync_base(cx);
    tint(cx);
}

/// Switches to `mode`, or follows the window's appearance when `None`.
pub(crate) fn apply(mode: Option<ThemeMode>, window: Option<&mut Window>, cx: &mut App) {
    match mode {
        Some(mode) => Theme::change(mode, window, cx),
        None => Theme::sync_system_appearance(window, cx),
    }
    tint(cx);
}

fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}

fn tint(cx: &mut App) {
    let theme = Theme::global_mut(cx);
    let dark = theme.is_dark();
    let (primary, hover, active) = if dark {
        (0x3b82f6, 0x60a5fa, 0x2563eb)
    } else {
        (0x2563eb, 0x1d4ed8, 0x1e40af)
    };
    let white = color(0xffffff);
    theme.colors.primary = color(primary);
    theme.colors.primary_hover = color(hover);
    theme.colors.primary_active = color(active);
    theme.colors.primary_foreground = white;
    theme.colors.button_primary = color(primary);
    theme.colors.button_primary_hover = color(hover);
    theme.colors.button_primary_active = color(active);
    theme.colors.button_primary_foreground = white;
    theme.colors.sidebar_primary = color(primary);
    theme.colors.sidebar_primary_foreground = white;
    theme.colors.link = color(if dark {
        0x60a5fa
    } else {
        0x2563eb
    });
    theme.colors.ring = color(primary).opacity(0.5);
    theme.colors.selection = color(primary).opacity(if dark {
        0.35
    } else {
        0.22
    });
    // Components read the resolved tokens, which are derived from the colors.
    theme.tokens = ThemeTokens::from(&theme.colors);
    Theme::sync_base(cx);
}
