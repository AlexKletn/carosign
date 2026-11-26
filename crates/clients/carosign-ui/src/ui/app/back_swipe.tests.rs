use obscur_kit::geometry::Point;

use obscur_kit::interaction::gesture::{BackGesture, BackGesturePhase, BackGestureUpdate};
use obscur_kit::interaction::pointer::{
    PointerDevice, PointerEvent, PointerEventKind, PointerId, WheelEvent, WheelPhase,
};

fn completed(update: Option<BackGestureUpdate>) -> bool {
    update.is_some_and(|event| event.phase == BackGesturePhase::Completed)
}

fn wheel(phase: WheelPhase, x: f32, y: f32) -> WheelEvent {
    WheelEvent::new(Point::new(0.0, 0.0), y)
        .with_delta_x(x)
        .with_phase(phase)
        .as_trusted()
}

#[test]
fn rightward_swipe_commits_once_at_release_in_logical_pixels() {
    let mut swipe = BackGesture::default();
    assert!(!completed(
        swipe.update_wheel(wheel(WheelPhase::Started, -50.0, 0.0), 2.0)
    ));
    assert!(!completed(
        swipe.update_wheel(wheel(WheelPhase::Moved, -100.0, 8.0), 2.0)
    ));
    assert!(completed(
        swipe.update_wheel(wheel(WheelPhase::Ended, 0.0, 0.0), 2.0)
    ));
    assert!(!completed(
        swipe.update_wheel(wheel(WheelPhase::Ended, 0.0, 0.0), 2.0)
    ));
}

#[test]
fn rejects_reverse_diagonal_discrete_and_cancelled_sequences() {
    for (x, y) in [(100.0, 0.0), (-100.0, 90.0), (-30.0, 0.0)] {
        let mut swipe = BackGesture::default();
        assert!(!completed(
            swipe.update_wheel(wheel(WheelPhase::Started, x, y), 1.0)
        ));
        assert!(!completed(
            swipe.update_wheel(wheel(WheelPhase::Ended, 0.0, 0.0), 1.0)
        ));
    }
    let mut swipe = BackGesture::default();
    assert!(!completed(swipe.update_wheel(
        wheel(WheelPhase::Started, -100.0, 0.0).in_lines(),
        1.0
    )));
    assert!(!completed(
        swipe.update_wheel(wheel(WheelPhase::Ended, 0.0, 0.0), 1.0)
    ));
    assert!(!completed(
        swipe.update_wheel(wheel(WheelPhase::Started, -100.0, 0.0), 1.0)
    ));
    assert!(!completed(
        swipe.update_wheel(wheel(WheelPhase::Cancelled, 0.0, 0.0), 1.0)
    ));
    assert!(!completed(
        swipe.update_wheel(wheel(WheelPhase::Ended, 0.0, 0.0), 1.0)
    ));
}

#[test]
fn stage_accepts_phased_scroll_and_only_two_finger_native_swipes() {
    use std::{cell::Cell, num::NonZeroUsize, rc::Rc};

    use carosign_l10n::Localizer;
    use obscur_kit::{
        AppControl,
        bootstrap::CssPipeline,
        geometry::Size,
        identity::{InternedSymbol, RuntimeId},
        interaction::{
            event::{EventTrust, UiEvent},
            gesture::{GestureSource, TouchpadGestureDelta, TouchpadGestureEvent},
        },
        runtime::{UiRuntime, frame::FrameReason},
    };

    use crate::ui::{
        app::{Dashboard, screen_transition::ScreenTransition},
        page::Page,
    };

    let control = AppControl::new();
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(540.0, 580.0));
    crate::ui::app::window_theme::apply(&control, Default::default());
    control.apply(&mut runtime, &mut CssPipeline::new());

    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    let page = Rc::new(Cell::new(Page::Platform));
    let rendered = Rc::clone(&page);
    let mut transition = ScreenTransition::default();
    runtime.set_root_view(move || {
        let page = rendered.get();
        transition.update(
            page as u64,
            dashboard.view(page, None, true, false),
            false,
            false,
        );
        transition.view(540.0, true, &control)
    });
    let mut swipe =
        BackGesture::default().with_touchpad_finger_count(NonZeroUsize::new(2).unwrap());
    let source = Rc::new(Cell::new(None));
    let seen_source = Rc::clone(&source);
    let selected = Rc::clone(&page);
    runtime.on_command(
        InternedSymbol::named("navigation_swipe"),
        move |event: &mut UiEvent| {
            if let Some(update) = swipe.update(event, 1.0)
                && update.phase == BackGesturePhase::Completed
            {
                seen_source.set(Some(update.source));
                selected.set(Page::Sections);
                event.prevent_default();
            }
        },
    );
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();
    let at = Point::new(200.0, 100.0);
    runtime.handle_wheel(wheel(WheelPhase::Started, -40.0, 0.0).at(at));
    runtime.handle_wheel(wheel(WheelPhase::Moved, -40.0, 0.0).at(at));
    assert!(page.get() == Page::Platform);
    assert!(
        runtime
            .handle_wheel(wheel(WheelPhase::Ended, 0.0, 0.0).at(at))
            .is_some()
    );
    assert!(page.get() == Page::Sections);
    assert_eq!(source.get(), Some(GestureSource::PhasedScroll));

    for fingers in [0, 1, 3, 4, 2] {
        page.set(Page::Platform);
        source.set(None);
        for (phase, x) in [
            (BackGesturePhase::Started, 0.0),
            (BackGesturePhase::Updated, 80.0),
            (BackGesturePhase::Completed, 0.0),
        ] {
            runtime.handle_touchpad_gesture(TouchpadGestureEvent {
                phase,
                position: at,
                finger_count: NonZeroUsize::new(fingers),
                delta: TouchpadGestureDelta::Swipe(Point::new(x, 0.0)),
                trust: EventTrust::Trusted,
                timestamp_micros: 42,
            });
            if phase != BackGesturePhase::Completed {
                assert!(page.get() == Page::Platform);
            }
        }
        if fingers == 2 {
            assert!(page.get() == Page::Sections);
            assert_eq!(source.get(), Some(GestureSource::Touchpad));
        } else {
            assert!(page.get() == Page::Platform);
            assert_eq!(source.get(), None);
        }
    }
}

fn touch(id: u64, kind: PointerEventKind, x: f32, y: f32) -> PointerEvent {
    PointerEvent::new(
        PointerId::touch(id),
        PointerDevice::Touch,
        kind,
        Point::new(x, y),
    )
    .as_trusted()
}

#[test]
fn a_touch_from_the_left_edge_tracks_in_viewport_pixels_and_commits_on_release() {
    use PointerEventKind::{Down, Move, Up};

    let mut swipe = BackGesture::default();
    assert!(!completed(swipe.update_touch(touch(1, Down, 12.0, 80.0))));
    assert!(!completed(swipe.update_touch(touch(1, Move, 42.0, 83.0))));
    assert_eq!(swipe.offset(), Some(30.0));
    assert!(completed(swipe.update_touch(touch(1, Up, 96.0, 83.0))));
    assert_eq!(swipe.offset(), None);
    assert!(!completed(swipe.update_touch(touch(1, Up, 96.0, 83.0))));
}

#[test]
fn interior_vertical_and_cancelled_touch_sequences_leave_navigation_alone() {
    use PointerEventKind::{Cancel, Down, Move, Up};

    let mut swipe = BackGesture::default();
    swipe.update_touch(touch(1, Down, 64.0, 80.0));
    assert!(!completed(swipe.update_touch(touch(1, Up, 160.0, 80.0))));
    swipe.update_touch(touch(1, Down, 12.0, 80.0));
    assert!(!completed(swipe.update_touch(touch(1, Move, 16.0, 98.0))));
    assert!(!completed(swipe.update_touch(touch(1, Up, 160.0, 98.0))));
    swipe.update_touch(touch(1, Down, 12.0, 80.0));
    swipe.update_touch(touch(1, Move, 110.0, 80.0));
    assert!(!completed(
        swipe.update_touch(touch(1, Cancel, 110.0, 80.0))
    ));
    assert_eq!(swipe.offset(), None);
    assert!(!completed(swipe.update_touch(touch(1, Up, 110.0, 80.0))));
}

#[test]
fn compact_stage_routes_touch_to_the_back_gesture() {
    use std::{cell::Cell, rc::Rc};

    use carosign_l10n::Localizer;
    use obscur_kit::{
        AppControl,
        bootstrap::CssPipeline,
        geometry::Size,
        identity::{InternedSymbol, RuntimeId},
        interaction::event::UiEvent,
        runtime::{UiRuntime, frame::FrameReason},
    };

    use crate::ui::{
        app::{Dashboard, screen_transition::ScreenTransition},
        page::Page,
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

    let seen = Rc::new(Cell::new((false, None)));
    let updates = Rc::clone(&seen);
    let mut swipe = BackGesture::default();
    runtime.on_command(
        InternedSymbol::named("navigation_swipe"),
        move |event: &mut UiEvent| {
            if event.origin == obscur_kit::interaction::event::EventOrigin::Gesture
                && let Some(pointer) = event.pointer()
            {
                let update = swipe.update_touch(pointer);
                updates.set((completed(update), swipe.offset()));
                if update.is_some() {
                    event.prevent_default();
                }
            }
        },
    );
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();

    runtime.handle_pointer(touch(7, PointerEventKind::Down, 12.0, 200.0));
    runtime.handle_pointer(touch(7, PointerEventKind::Move, 38.0, 202.0));
    assert_eq!(seen.get(), (false, Some(26.0)));
    runtime.handle_pointer(touch(7, PointerEventKind::Up, 106.0, 202.0));
    assert_eq!(seen.get(), (true, None));
}
