use std::{cell::RefCell, rc::Rc};

use obscur_kit::{
    bootstrap::CssPipeline,
    geometry::{Point, Size},
    identity::RuntimeId,
    interaction::pointer::{PointerDevice, PointerEvent, PointerEventKind, PointerId},
    runtime::{UiRuntime, frame::FrameReason},
    system::appearance::{WindowButtonLayout, WindowDecoration},
};

use super::*;

#[test]
fn desktop_controls_leave_room_for_content_and_dragging() {
    let control = AppControl::new();
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(480.0, 360.0));
    let mut css = CssPipeline::new();
    crate::ui::app::window_theme::apply(&control, Default::default());
    control.apply(&mut runtime, &mut css);

    let localizer = Localizer::from_environment();
    let decoration = WindowDecoration {
        button_layout: WindowButtonLayout {
            left: Some(vec![WindowButton::Close, WindowButton::Minimize]),
            right: Some(vec![WindowButton::Maximize]),
        },
        button_icons: Default::default(),
    };
    use crate::ui::app::{Dashboard, screen_transition::ScreenTransition};
    use crate::ui::page::Page;

    let mut stage = ScreenTransition::default();
    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    stage.update(
        1,
        dashboard.view(Page::Platform, None, true, false),
        false,
        false,
    );
    stage.set_window_controls(view(
        &SystemAppearance {
            icon_theme: Some("Adwaita".into()),
            window_decoration: Some(decoration),
            ..SystemAppearance::default()
        },
        false,
        2,
        &localizer,
    ));
    let stage_control = control.clone();
    runtime.set_root_view(move || stage.view(480.0, true, &stage_control));
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();
    control.apply(&mut runtime, &mut css);

    let snapshot = runtime.inspect_snapshot();
    let boxes = |class: &str| {
        snapshot
            .nodes
            .iter()
            .filter(|node| node.classes.iter().any(|name| name == class))
            .map(|node| runtime.layout_boxes().get(&node.node).copied().unwrap())
            .collect::<Vec<_>>()
    };
    let buttons = boxes("window-control");
    let header = boxes("header")[0];
    let back = boxes("header-back")[0];

    assert_eq!(buttons.len(), 3);
    assert!(buttons[0].origin().x < buttons[1].origin().x);
    assert!(buttons[1].origin().x < buttons[2].origin().x);
    assert_eq!(buttons[0].origin().x, 10.0);
    assert!(buttons[1].origin().x + buttons[1].size().width <= back.origin().x);
    assert!(header.size().width > 200.0);
    assert_eq!(header.origin().y, 0.0);
    let received = Rc::new(RefCell::new(Vec::new()));
    let commands = ["window_close", "window_minimize", "window_maximize"];
    for command in commands {
        let received = Rc::clone(&received);
        runtime.on_command(InternedSymbol::named(command), move || {
            received.borrow_mut().push(command);
        });
    }

    for (index, button) in buttons.into_iter().enumerate() {
        assert_eq!(button.origin().y, 12.0);
        assert_eq!(button.size(), Size::new(28.0, 28.0));
        let center = Point::new(button.origin().x + 14.0, button.origin().y + 14.0);
        let id = PointerId::from_raw(index as u64 + 1);
        for kind in [PointerEventKind::Down, PointerEventKind::Up] {
            runtime.handle_pointer(
                PointerEvent::new(id, PointerDevice::Touch, kind, center).as_trusted(),
            );
        }
    }
    assert_eq!(*received.borrow(), commands);

    let navigation = Rc::clone(&received);
    runtime.on_command(InternedSymbol::named("navigate_back"), move || {
        navigation.borrow_mut().push("navigate_back");
    });
    let drag_requests = Rc::new(RefCell::new(0));
    let handled = Rc::clone(&drag_requests);
    runtime.on_command(
        InternedSymbol::named("window_move"),
        move |event: &mut UiEvent| {
            if is_window_drag_press(event) {
                *handled.borrow_mut() += 1;
            }
        },
    );
    let press = |runtime: &mut UiRuntime, point, button| {
        for kind in [PointerEventKind::Down, PointerEventKind::Up] {
            runtime.handle_pointer(
                PointerEvent::new(PointerId::MOUSE, PointerDevice::Mouse, kind, point)
                    .with_button(button)
                    .as_trusted(),
            );
        }
    };
    let title = Point::new(header.origin().x + header.size().width / 2.0, 24.0);
    let back_button = Point::new(back.origin().x + back.size().width / 2.0, 24.0);
    let close_button = Point::new(24.0, 24.0);
    press(&mut runtime, title, PointerButton::Primary);
    assert_eq!(*drag_requests.borrow(), 1);
    press(&mut runtime, title, PointerButton::Secondary);
    press(&mut runtime, back_button, PointerButton::Primary);
    assert_eq!(received.borrow().last(), Some(&"navigate_back"));
    press(&mut runtime, close_button, PointerButton::Primary);
    assert_eq!(*drag_requests.borrow(), 1);
    assert_eq!(received.borrow().last(), Some(&"window_close"));
    for point in [title, back_button, close_button] {
        for kind in [PointerEventKind::Down, PointerEventKind::Up] {
            runtime.handle_pointer(
                PointerEvent::new(PointerId::from_raw(20), PointerDevice::Touch, kind, point)
                    .as_trusted(),
            );
        }
    }
    assert_eq!(*drag_requests.borrow(), 2);
}

#[test]
fn missing_desktop_icon_uses_the_builtin_svg() {
    use obscur_kit::resources::{port::ResourceLoader, source::ResourceSource};

    let loader = crate::ui::widgets::WidgetImages;
    let themed =
        ResourceSource::file("window-theme-icon/1/Adwaita/carosign-no-such-icon/window-close.svg");
    let builtin = ResourceSource::file("window-close.svg");

    assert!(loader.can_load(&themed));
    assert_eq!(
        loader.load(&themed).unwrap().bytes(),
        loader.load(&builtin).unwrap().bytes(),
    );
}

#[test]
fn wide_sidebar_title_moves_below_left_window_controls() {
    use crate::ui::{
        app::{Dashboard, screen_transition::ScreenTransition},
        page::Page,
    };

    let control = AppControl::new();
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(780.0, 580.0));
    let mut css = CssPipeline::new();
    crate::ui::app::window_theme::apply(&control, Default::default());
    control.apply(&mut runtime, &mut css);

    let localizer = Localizer::from_environment();
    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    let mut stage = ScreenTransition::default();
    stage.update(
        1,
        dashboard.view(Page::Platform, None, false, false),
        false,
        false,
    );
    let appearance = SystemAppearance {
        window_decoration: Some(WindowDecoration {
            button_layout: WindowButtonLayout {
                left: Some(vec![
                    WindowButton::Close,
                    WindowButton::Minimize,
                    WindowButton::Maximize,
                ]),
                right: Some(vec![]),
            },
            button_icons: Default::default(),
        }),
        ..SystemAppearance::default()
    };
    stage.set_window_controls(view(&appearance, false, 1, &localizer));
    let stage_control = control.clone();
    runtime.set_root_view(move || stage.view(780.0, false, &stage_control));
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();

    let snapshot = runtime.inspect_snapshot();
    let bounds = |class: &str| {
        let node = snapshot
            .nodes
            .iter()
            .find(|node| node.classes.iter().any(|name| name == class))
            .unwrap();
        runtime.layout_boxes().get(&node.node).copied().unwrap()
    };
    let buttons = bounds("window-controls-left");
    let title = bounds("sidebar-title");
    assert_eq!(buttons.origin().x, 10.0);
    assert!(title.origin().y >= buttons.origin().y + buttons.size().height);
    assert_eq!(bounds("header").origin().y, 0.0);
}
