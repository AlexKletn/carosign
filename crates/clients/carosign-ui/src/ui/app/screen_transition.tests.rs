use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use super::super::screen_pose::UNDERLAY_SCALE;

use carosign_l10n::Localizer;
use obscur_kit::bootstrap::CssPipeline;
use obscur_kit::geometry::Size;
use obscur_kit::identity::RuntimeId;
use obscur_kit::runtime::UiRuntime;
use obscur_kit::scene::command::PaintCommand;
use obscur_kit::style::{
    computed::ComputedValue,
    property::{PropertyId, StyleColor},
};
use obscur_kit::view::view::STYLE;

use super::*;
use crate::ui::{app::Dashboard, page::Page};

const DURATION: Duration = Duration::from_millis(250);

#[test]
fn stage_preserves_layout_and_requests_frames_until_transition_finishes() {
    check_stage(true, 680.0);
}

#[test]
fn wide_stage_keeps_sidebar_at_window_edge() {
    check_stage(false, 780.0);
}

fn check_stage(compact: bool, width: f32) {
    let control = AppControl::new();
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(width, 580.0));
    let mut css = CssPipeline::new();
    crate::ui::app::window_theme::apply(&control, Default::default());
    control.apply(&mut runtime, &mut css);

    let stage_control = control.clone();
    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    let mut transition = ScreenTransition::default();
    transition.update(
        1,
        dashboard.view(Page::Sections, None, compact, true),
        false,
        false,
    );
    transition.update(
        2,
        dashboard.view(Page::Platform, None, compact, true),
        true,
        false,
    );
    runtime.set_root_view(move || transition.view(width, compact, &stage_control));

    runtime.request_frame(FrameReason::UiCommit);
    let mut previous_scale = 1.0;
    let mut sidebar_node = None;
    let mut last_elapsed = Duration::ZERO;
    for elapsed in [
        Duration::ZERO,
        Duration::from_millis(16),
        Duration::from_millis(32),
        DURATION / 2,
        DURATION - Duration::from_millis(16),
        DURATION,
    ] {
        runtime.advance_motion((elapsed - last_elapsed).as_secs_f32());
        last_elapsed = elapsed;
        assert!(runtime.has_pending_frame());
        if elapsed > Duration::ZERO && elapsed < DURATION {
            assert!(runtime.has_pending_timeline_frame());
        }
        runtime.run_due_frame_reactive().unwrap();
        control.apply(&mut runtime, &mut css);

        let nodes = runtime.inspect_snapshot().nodes;
        for node in &nodes {
            if node.classes.iter().any(|class| class == "page")
                || compact
                    && node.classes.iter().any(|class| class == "sidebar")
                    && elapsed < DURATION
                    && runtime
                        .layout_boxes()
                        .get(&node.node)
                        .is_some_and(|bounds| bounds.size().width >= width)
            {
                let color = runtime
                    .computed_style(node.node)
                    .unwrap()
                    .get(PropertyId::BackgroundColor)
                    .and_then(ComputedValue::as_color);
                assert_eq!(color, Some(StyleColor::TRANSPARENT));
            }
        }

        let layers: Vec<_> = nodes
            .iter()
            .filter(|node| node.classes.iter().any(|class| class == "screen-layer"))
            .collect();
        assert_eq!(layers.len(), if elapsed == DURATION { 1 } else { 2 });
        if compact {
            let sidebars = nodes
                .iter()
                .filter(|node| node.classes.iter().any(|class| class == "sidebar"))
                .count();
            assert_eq!(sidebars, usize::from(elapsed < DURATION));
        }
        if elapsed > Duration::ZERO && elapsed < DURATION {
            let scale = runtime
                .scene()
                .root()
                .commands()
                .iter()
                .find_map(|command| match command {
                    PaintCommand::PushTransform(transform)
                        if transform.coeffs[0] < 1.0 && transform.coeffs[3] < 1.0 =>
                    {
                        Some(transform.coeffs[0])
                    }
                    _ => None,
                })
                .expect("outgoing scale in the rendered frame");
            assert!(scale < previous_scale && scale >= UNDERLAY_SCALE);
            previous_scale = scale;
        }
        if elapsed == DURATION / 2 {
            let shadow = nodes
                .iter()
                .find(|node| {
                    node.classes
                        .iter()
                        .any(|class| class == "screen-transition-shadow")
                })
                .unwrap();
            let bounds = runtime.layout_boxes().get(&shadow.node).unwrap();
            assert_eq!(bounds.size(), Size::new(22.0, 580.0));

            let boundary = if compact {
                width / 2.0
            } else {
                (width + WIDE_SIDEBAR_WIDTH) / 2.0
            };
            let mut clips = Vec::new();
            let mut clips_lower_layer = false;
            for command in runtime.scene().root().commands() {
                match command {
                    PaintCommand::PushClip(clip) => clips.push(
                        clip.bounds()
                            .is_some_and(|bounds| bounds.size == Size::new(boundary, 580.0)),
                    ),
                    PaintCommand::PopClip => {
                        clips.pop();
                    }
                    PaintCommand::PushTransform(transform) if transform.coeffs[0] < 1.0 => {
                        clips_lower_layer |= clips.contains(&true);
                    }
                    PaintCommand::PushTransform(transform)
                        if transform.coeffs[4] == width - boundary =>
                    {
                        assert!(
                            !clips.contains(&true),
                            "upper screen is outside the lower screen clip"
                        );
                    }
                    _ => {}
                }
            }
            assert!(clips_lower_layer);
        }
        if elapsed == DURATION / 2 && compact {
            let mut transforms = Vec::new();
            let mut header_count = 0;
            for command in runtime.scene().root().commands() {
                match command {
                    PaintCommand::PushTransform(transform) => transforms.push(*transform),
                    PaintCommand::PopTransform => {
                        transforms.pop();
                    }
                    PaintCommand::DrawText { origin, .. } if origin.y < 40.0 => {
                        header_count += 1;
                        assert!(
                            !transforms.is_empty(),
                            "header painted outside its screen transform"
                        );
                    }
                    _ => {}
                }
            }
            assert!(header_count >= 2);
        }
        let divider = nodes
            .iter()
            .find(|node| node.classes.iter().any(|class| class == "content-divider"));
        if compact {
            assert!(divider.is_none());
        } else {
            let bounds = runtime.layout_boxes().get(&divider.unwrap().node).unwrap();
            assert_eq!(bounds.origin().x, WIDE_SIDEBAR_WIDTH);
            assert_eq!(bounds.size(), Size::new(1.0, 580.0));
        }
        if !compact {
            let sidebar = nodes
                .iter()
                .find(|node| {
                    node.classes
                        .iter()
                        .any(|class| class == "stationary-sidebar")
                })
                .unwrap();
            assert_eq!(*sidebar_node.get_or_insert(sidebar.node), sidebar.node);
            let bounds = runtime.layout_boxes().get(&sidebar.node).unwrap();
            assert_eq!(bounds.origin().x, 0.0);
            assert_eq!(bounds.size(), Size::new(WIDE_SIDEBAR_WIDTH, 580.0));
        }
        for layer in layers {
            let bounds = runtime.layout_boxes().get(&layer.node).unwrap();
            assert_eq!(bounds.origin().x, 0.0);
            assert_eq!(bounds.size(), Size::new(width, 580.0));
        }
        if !compact {
            for pane in nodes
                .iter()
                .filter(|node| node.classes.iter().any(|class| class == "content-pane"))
            {
                let bounds = runtime.layout_boxes().get(&pane.node).unwrap();
                assert_eq!(bounds.origin().x, WIDE_SIDEBAR_WIDTH);
                assert_eq!(bounds.size(), Size::new(width - WIDE_SIDEBAR_WIDTH, 580.0));
            }
        }
    }
}

#[test]
fn reversing_mid_transition_preserves_the_visual_positions() {
    let control = AppControl::new();
    let mut transition = ScreenTransition::default();
    transition.update(1, View::default(), false, false);
    transition.update(2, View::default(), true, false);
    let motion = transition.motion.as_mut().unwrap();
    motion.start = 0.25;
    motion.progress.set(0.25);
    let forward = transition.view(680.0, true, &control);

    transition.update(1, View::default(), true, true);
    let backwards = transition.view(680.0, true, &control);

    let styles = |view: View| {
        let ViewNode::Element { children, .. } = view.into_single_node().unwrap() else {
            panic!("expected stage");
        };
        children
            .into_nodes()
            .into_iter()
            .flat_map(|viewport| {
                let ViewNode::Element { children, .. } = viewport else {
                    panic!("expected viewport")
                };
                children.into_nodes()
            })
            .map(|layer| {
                let ViewNode::Element { props, .. } = layer else {
                    panic!("expected layer");
                };
                props.into_iter().find(|prop| prop.name() == STYLE).unwrap()
            })
            .collect::<Vec<_>>()
    };
    let mut expected = styles(forward);
    expected.reverse();
    assert_eq!(styles(backwards), expected);
}

#[test]
fn swipe_distance_controls_the_slide_and_release_keeps_its_position() {
    let control = AppControl::new();
    let mut stage = ScreenTransition::default();
    stage.update(2, View::empty(), false, false);
    let position = |stage: &mut ScreenTransition| {
        let ViewNode::Element { children, .. } = stage
            .view(600.0, true, &control)
            .into_single_node()
            .unwrap()
        else {
            panic!("expected stage");
        };
        children
            .into_nodes()
            .into_iter()
            .flat_map(|viewport| {
                let ViewNode::Element { children, .. } = viewport else {
                    panic!("expected viewport")
                };
                children.into_nodes()
            })
            .find_map(|node| {
                let ViewNode::Element { props, .. } = node else {
                    return None;
                };
                props.into_iter().find_map(|prop| {
                    let ViewValue::Text(style) = prop.value() else {
                        return None;
                    };
                    style
                        .strip_prefix("transform: translateX(")?
                        .strip_suffix("px)")?
                        .parse::<f32>()
                        .ok()
                })
            })
    };
    stage.track_back(32.0, 600.0, true, || (1, View::empty()));
    assert_eq!(position(&mut stage), Some(32.0));

    stage.release_back();
    assert_eq!(position(&mut stage), Some(32.0));
    stage.motion.as_ref().unwrap().progress.set(1.0);
    assert_eq!(position(&mut stage), None);

    stage.track_back(110.0, 600.0, true, || (1, View::empty()));
    assert_eq!(position(&mut stage), Some(110.0));
    stage.update(1, View::empty(), true, true);
    assert_eq!(position(&mut stage), Some(110.0));
    stage.motion.as_ref().unwrap().progress.set(1.0);
    assert_eq!(position(&mut stage), None);
}

#[test]
fn mouse_back_release_bubbles_from_screen_content() {
    use obscur_kit::geometry::Point;
    use obscur_kit::interaction::{
        event::UiEvent,
        pointer::{PointerButton, PointerDevice, PointerEvent, PointerEventKind, PointerId},
    };

    let control = AppControl::new();
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(540.0, 580.0));
    crate::ui::app::window_theme::apply(&control, Default::default());
    control.apply(&mut runtime, &mut CssPipeline::new());
    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    let mut stage = ScreenTransition::default();
    stage.update(
        2,
        dashboard.view(Page::Platform, None, true, false),
        false,
        false,
    );
    runtime.set_root_view(move || stage.view(540.0, true, &control));
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();

    let released = Rc::new(Cell::new(None));
    let seen = Rc::clone(&released);
    runtime.on_command(
        InternedSymbol::named("navigation_swipe"),
        move |event: &mut UiEvent| {
            if let Some(pointer) = event
                .pointer()
                .filter(|pointer| pointer.kind() == PointerEventKind::Up)
            {
                seen.set(pointer.button());
            }
        },
    );
    let event = |kind| {
        PointerEvent::new(
            PointerId::MOUSE,
            PointerDevice::Mouse,
            kind,
            Point::new(200.0, 100.0),
        )
        .with_button(PointerButton::Back)
        .as_trusted()
    };
    runtime.handle_pointer(event(PointerEventKind::Down));
    assert_eq!(released.get(), None);
    runtime.handle_pointer(event(PointerEventKind::Up));
    assert_eq!(released.get(), Some(PointerButton::Back));
}

#[test]
fn window_root_follows_the_system_theme_radius() {
    use obscur_kit::geometry::CornerRadii;
    use obscur_kit::system::appearance::SystemAppearance;

    let width = 540.0;
    let control = AppControl::new();
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(width, 580.0));
    let mut css = CssPipeline::new();
    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    let mut stage = ScreenTransition::default();
    stage.update(
        1,
        dashboard.view(Page::Platform, None, true, false),
        false,
        false,
    );
    let stage_control = control.clone();
    let maximized = Rc::new(Cell::new(false));
    let window_state = Rc::clone(&maximized);
    runtime.set_root_view(move || {
        stage.set_window_maximized(window_state.get());
        stage.view(width, true, &stage_control)
    });

    for (radius, is_maximized, expected) in [
        (None, false, 0.0),
        (Some(15.0), false, 15.0),
        (Some(15.0), true, 0.0),
        (Some(15.0), false, 15.0),
        (Some(0.0), false, 0.0),
        (Some(8.0), false, 8.0),
    ] {
        maximized.set(is_maximized);
        crate::ui::app::window_theme::apply(
            &control,
            SystemAppearance {
                window_radius: radius,
                ..SystemAppearance::default()
            },
        );
        control.apply(&mut runtime, &mut css);
        runtime.request_frame(FrameReason::UiCommit);
        runtime.run_due_frame_reactive().unwrap();
        assert_eq!(
            runtime.viewport_corner_radii(),
            CornerRadii::uniform(expected)
        );
    }
}

#[test]
fn compact_swipe_translates_visible_page_content_with_the_fingers() {
    use obscur_kit::{
        geometry::CornerRadii,
        scene::clip::{Clip, ClipShape},
        system::appearance::SystemAppearance,
    };

    let control = AppControl::new();
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(540.0, 580.0));
    crate::ui::app::window_theme::apply(
        &control,
        SystemAppearance {
            window_radius: Some(14.0),
            ..Default::default()
        },
    );
    control.with_runtime(|runtime| runtime.set_surface(Size::new(864.0, 928.0), 1.6));
    let mut css = CssPipeline::new();
    control.apply(&mut runtime, &mut css);

    let localizer = Localizer::from_environment();
    let loading = localizer.text("ui-fingerprint-loading");
    let dashboard = Dashboard::new(localizer).unwrap();
    let offset = Rc::new(Cell::new(None));
    let tracked_offset = Rc::clone(&offset);
    let mut stage = ScreenTransition::default();
    stage.update(
        1,
        dashboard.view(Page::Sections, None, true, false),
        false,
        false,
    );
    stage.update(
        2,
        dashboard.view(Page::Fingerprints, None, true, true),
        true,
        false,
    );
    let stage_control = control.clone();
    runtime.set_root_view(move || {
        if let Some(offset) = tracked_offset.get() {
            stage.track_back(offset, 540.0, true, || {
                (1, dashboard.view(Page::Sections, None, true, false))
            });
        }
        stage.view(540.0, true, &stage_control)
    });

    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();
    control.apply(&mut runtime, &mut css);
    runtime.advance_motion(DURATION.as_secs_f32());
    control.apply(&mut runtime, &mut css);
    runtime.run_due_frame_reactive().unwrap();
    control.apply(&mut runtime, &mut css);

    for distance in [0.0_f32, 45.0, 115.0] {
        offset.set((distance != 0.0).then_some(distance));
        runtime.request_frame(FrameReason::UiCommit);
        runtime.run_due_frame_reactive().unwrap();
        let mut transforms = Vec::new();
        let mut body_runs = 0;
        let mut rounded_moving_clip = false;
        let mut rounded_stationary_clip = false;
        let right_radius = CornerRadii::uniform(14.0).top_right;
        let right_edge_radii = CornerRadii {
            top_right: right_radius,
            bottom_right: right_radius,
            ..Default::default()
        };
        for command in runtime.scene().root().commands() {
            match command {
                PaintCommand::PushTransform(transform) => transforms.push(*transform),
                PaintCommand::PopTransform => {
                    transforms.pop();
                }
                PaintCommand::PushClip(Clip::Shape(ClipShape::RoundedRect { radii, .. }))
                    if transforms.iter().map(|t| t.coeffs[4]).sum::<f32>() > 1.0 =>
                {
                    rounded_moving_clip |= *radii == CornerRadii::uniform(14.0);
                }
                PaintCommand::PushClip(Clip::Shape(ClipShape::RoundedRect { rect, radii }))
                    if rect.size == Size::new(540.0, 580.0) =>
                {
                    rounded_stationary_clip |= *radii == right_edge_radii;
                }
                PaintCommand::DrawText { run, .. } if run.text() == loading => {
                    let shift = transforms.iter().map(|t| t.coeffs[4]).sum::<f32>();
                    assert!(
                        (shift - distance).abs() < 0.1,
                        "body text shifted {shift}px at a swipe of {distance}px"
                    );
                    body_runs += 1;
                }
                _ => {}
            }
        }
        assert_eq!(rounded_moving_clip, distance > 0.0);
        assert_eq!(rounded_stationary_clip, distance > 0.0);
        assert!(
            body_runs > 0,
            "expected visible page content at {distance}px"
        );
    }
}
