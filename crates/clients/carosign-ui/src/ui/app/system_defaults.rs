use obscur_kit::{
    scene::color::Color,
    system::appearance::{ColorScheme, SystemAppearance, SystemFont, SystemPalette},
};

struct FallbackPalette {
    window_background: u32,
    window_foreground: u32,
    view_background: u32,
    view_foreground: u32,
    selection_background: u32,
    selection_foreground: u32,
    accent: u32,
}

include!(concat!(env!("OUT_DIR"), "/system_defaults.rs"));

pub(super) fn with_fallbacks(appearance: SystemAppearance) -> SystemAppearance {
    let scheme = appearance.color_scheme.unwrap_or(COLOR_SCHEME);
    let colors = match scheme {
        ColorScheme::Dark => DARK,
        ColorScheme::Light => LIGHT,
    };
    let mut appearance = appearance.merge_under(&SystemAppearance {
        color_scheme: Some(scheme),
        accent: Some(rgb(colors.accent)),
        window_radius: Some(WINDOW_RADIUS),
        palette: SystemPalette {
            window_background: Some(rgb(colors.window_background)),
            window_foreground: Some(rgb(colors.window_foreground)),
            view_background: Some(rgb(colors.view_background)),
            view_foreground: Some(rgb(colors.view_foreground)),
            selection_background: Some(rgb(colors.selection_background)),
            selection_foreground: Some(rgb(colors.selection_foreground)),
        },
        ..SystemAppearance::default()
    });
    let scale = appearance
        .text_scale
        .filter(|scale| scale.is_finite() && *scale > 0.0)
        .unwrap_or(1.0);
    set_font_size(&mut appearance.font, FONT_FAMILY, FONT_SIZE, scale);
    set_font_size(
        &mut appearance.monospace_font,
        MONO_FAMILY,
        MONO_SIZE,
        scale,
    );
    appearance
}

fn set_font_size(font: &mut Option<SystemFont>, family: &str, default_pt: f32, scale: f32) {
    let font = font.get_or_insert_with(|| SystemFont::new(family, None));
    let points = font
        .size_pt
        .filter(|pt| pt.is_finite() && *pt > 0.0)
        .unwrap_or(default_pt);
    font.size_pt = Some(points * scale);
}

fn rgb(hex: u32) -> Color {
    let channel = |shift: u32| ((hex >> shift) & 0xff) as f32 / 255.0;
    Color::rgb(channel(16), channel(8), channel(0))
}
