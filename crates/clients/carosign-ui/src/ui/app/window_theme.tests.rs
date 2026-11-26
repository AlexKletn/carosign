use carosign_l10n::Localizer;
use obscur_kit::AppControl;
use obscur_kit::bootstrap::CssPipeline;
use obscur_kit::geometry::Size;
use obscur_kit::identity::RuntimeId;
use obscur_kit::runtime::UiRuntime;
use obscur_kit::scene::color::Color;
use obscur_kit::style::computed::ComputedValue;
use obscur_kit::style::property::{Length, PropertyId, StyleColor};
use obscur_kit::system::appearance::{ColorScheme, Contrast, SystemFont, SystemPalette};

use super::super::Dashboard;
use super::*;
use crate::ui::page::Page;

#[test]
fn system_palette_overrides_fallback_and_can_be_removed() {
    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(780.0, 580.0));
    let mut css = CssPipeline::new();
    let control = AppControl::new();
    runtime.set_root_view(move || dashboard.view(Page::Platform, None, false, false));

    for (appearance, background, background_alpha) in [
        (
            SystemAppearance {
                color_scheme: Some(ColorScheme::Light),
                ..SystemAppearance::default()
            },
            (246, 246, 246),
            1.0,
        ),
        (
            SystemAppearance {
                color_scheme: Some(ColorScheme::Light),
                palette: SystemPalette {
                    window_background: Some(Color::rgba(1.0, 0.0, 0.0, 0.75)),
                    view_background: Some(Color::rgb(0.0, 0.0, 1.0)),
                    ..SystemPalette::default()
                },
                ..SystemAppearance::default()
            },
            (255, 0, 0),
            0.75,
        ),
        (SystemAppearance::default(), (20, 20, 20), 1.0),
    ] {
        apply(&control, appearance);
        control.apply(&mut runtime, &mut css);
        runtime.run_due_frame_reactive().unwrap();
        let nodes = runtime.inspect_snapshot().nodes;
        for (class, (r, g, b), alpha) in [
            ("page", background, background_alpha),
            ("sidebar", (0, 0, 0), 0.0),
        ] {
            let node = nodes
                .iter()
                .find(|node| node.classes.iter().any(|name| name == class))
                .unwrap();
            let color = runtime
                .computed_style(node.node)
                .unwrap()
                .get(PropertyId::BackgroundColor)
                .and_then(ComputedValue::as_color);
            assert_eq!(
                color,
                Some(StyleColor::rgba(
                    r as f32 / 255.0,
                    g as f32 / 255.0,
                    b as f32 / 255.0,
                    alpha
                ))
            );
        }
    }
}

#[test]
fn system_preferences_restyle_text_selection_and_contrast() {
    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(780.0, 580.0));
    let mut css = CssPipeline::new();
    let control = AppControl::new();
    runtime.set_root_view(move || dashboard.view(Page::Policies, None, false, false));

    for high in [false, true, false] {
        apply(
            &control,
            SystemAppearance {
                contrast: Some(if high {
                    Contrast::High
                } else {
                    Contrast::Normal
                }),
                font: Some(SystemFont::new("sans-serif", Some(12.0))),
                text_scale: Some(if high { 1.5 } else { 1.0 }),
                palette: SystemPalette {
                    view_foreground: Some(Color::rgb(1.0, 1.0, 1.0)),
                    selection_background: Some(Color::rgb(0.0, 0.0, 1.0)),
                    ..SystemPalette::default()
                },
                ..SystemAppearance::default()
            },
        );
        control.apply(&mut runtime, &mut css);
        runtime.run_due_frame_reactive().unwrap();

        let nodes = runtime.inspect_snapshot().nodes;
        let style = |class| {
            let node = nodes
                .iter()
                .find(|node| node.classes.iter().any(|name| name == class))
                .unwrap();
            runtime.computed_style(node.node).unwrap()
        };
        assert_eq!(
            style("page")
                .get(PropertyId::FontSize)
                .and_then(ComputedValue::as_length),
            Some(Length::px(if high { 24.0 } else { 16.0 }))
        );
        assert_eq!(
            style("selected")
                .get(PropertyId::BackgroundColor)
                .and_then(ComputedValue::as_color),
            Some(StyleColor::rgba(0.0, 0.0, 1.0, 1.0))
        );
        assert_eq!(
            style("policy-primary")
                .get(PropertyId::BorderBottomColor)
                .and_then(ComputedValue::as_color)
                == Some(StyleColor::rgba(1.0, 1.0, 1.0, 1.0)),
            high
        );
    }
}
