use carosign_protocol::carosign::client::v1::{
    Authenticators, Health, ProviderInventory, ProviderPublication,
};
use carosign_protocol::carosign::common::v1::{AuthenticatorDescriptor, EnrollmentSummary};
use obscur_kit::view::view::{ViewFragment, ViewNode, has_outlet};

use super::*;
use crate::domain::QueryError;
use crate::ipc::SystemAccount;

#[test]
fn dashboard_projects_provider_devices_and_replaces_stale_inventory() {
    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    let id = vec![0xab; 16];
    let snapshot = Snapshot {
        enrollments: Ok(Vec::new()),
        health: Ok(Health {
            service_ready: true,
            available_authenticators: 7,
            ..Health::default()
        }),
        inventory: Ok(Authenticators {
            entries: vec![AuthenticatorDescriptor {
                authenticator_id: id.clone(),
                available: true,
                ..AuthenticatorDescriptor::default()
            }],
            providers: Some(ProviderInventory {
                publications: vec![ProviderPublication {
                    name: "example-provider".into(),
                    reachable: true,
                    authenticator_ids: vec![id],
                    ..ProviderPublication::default()
                }],
            }),
        }),
    };
    let view = dashboard.view(Page::Platform, Some(&snapshot), false, false);
    assert!(!has_outlet(view.root().nodes()));
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&"7"));
    let title = Localizer::from_environment().text("ui-settings-title");
    assert!(text.contains(&title.as_str()));
    let ready = Localizer::from_environment().text("ui-service-running");
    assert!(text.contains(&ready.as_str()));
    let heading = Localizer::from_environment().text("ui-platform-devices");
    let count_index = text.iter().position(|item| *item == "7").unwrap();
    let heading_index = text.iter().position(|item| *item == heading).unwrap();
    assert!(heading_index < count_index);
    assert!(text.contains(&"example-provider"));
    assert!(text.contains(&"abababababababababababababababab"));

    let offline = Snapshot {
        health: Err(QueryError::Unavailable),
        inventory: Err(QueryError::Unavailable),
        ..snapshot.clone()
    };
    for (snapshot, status_key) in [
        (None, "monitor-loading"),
        (Some(&offline), "monitor-unavailable"),
    ] {
        let view = dashboard.view(Page::Platform, snapshot, true, false);
        let mut text = Vec::new();
        collect_text(view.root(), &mut text);
        let status = Localizer::from_environment().text(status_key);
        assert_eq!(text.iter().filter(|item| **item == status).count(), 1);
        assert!(!text.contains(&"7"));
        assert!(!text.contains(&heading.as_str()));
    }

    let view = dashboard.view(
        Page::Platform,
        Some(&Snapshot {
            inventory: Err(QueryError::Unauthorized),
            ..snapshot
        }),
        false,
        false,
    );
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    let denied = Localizer::from_environment().text("ui-access-denied");
    assert!(text.contains(&denied.as_str()));
    assert!(text.contains(&heading.as_str()));
    assert!(text.contains(&"7"));
    assert!(!text.contains(&"example-provider"));
}

fn collect_text<'a>(fragment: &'a ViewFragment, text: &mut Vec<&'a str>) {
    for node in fragment.iter() {
        match node {
            ViewNode::Text { content, .. } => text.push(content),
            ViewNode::Element { children, .. } => collect_text(children, text),
        }
    }
}

#[test]
fn grouped_sidebar_keeps_policies_reachable_at_minimum_window_height() {
    use obscur_kit::{
        AppControl,
        bootstrap::{CssPipeline, KitBootstrap},
        geometry::Size,
        headless::input::{SelectorResolver, SelectorTarget},
        identity::RuntimeId,
        runtime::{UiRuntime, frame::FrameReason},
    };

    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(480.0, 360.0));
    KitBootstrap::new().install_default_stylesheet(&mut runtime);
    let control = AppControl::new();
    crate::ui::app::window_theme::apply(&control, Default::default());
    control.apply(&mut runtime, &mut CssPipeline::new());
    runtime.set_root_view(move || dashboard.view(Page::Sections, None, true, false));
    runtime.run_due_frame_reactive().unwrap();

    let resolve = |runtime: &UiRuntime, name| {
        SelectorResolver::new()
            .resolve(runtime, &SelectorTarget::test_id(name))
            .unwrap()
    };
    let semantics = runtime.semantic_snapshot();
    for id in ["face-navigation", "iris-navigation", "voice-navigation"] {
        let node = semantics.get(resolve(&runtime, id)).unwrap();
        assert!(node.state().disabled);
        assert!(node.name().is_some());
    }

    let policies = resolve(&runtime, "policies-navigation");
    let footer = resolve(&runtime, "service-restart");
    let footer_top = runtime.layout_boxes().get(&footer).unwrap().origin().y;
    runtime.scroll_into_view(policies);
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();
    let visible = runtime.semantic_snapshot();
    let policies_bottom = visible.get(policies).unwrap().bounds().unwrap().bottom();
    assert!(policies_bottom <= footer_top);
    assert_eq!(
        runtime.layout_boxes().get(&footer).unwrap().origin().y,
        footer_top
    );
}

#[test]
fn unavailable_rows_keep_disabled_semantics_without_opacity_groups_when_scrolled() {
    use obscur_kit::{
        AppControl,
        bootstrap::{CssPipeline, KitBootstrap},
        geometry::Size,
        headless::input::{SelectorResolver, SelectorTarget},
        identity::RuntimeId,
        runtime::{UiRuntime, frame::FrameReason},
    };

    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(752.0, 360.0));
    KitBootstrap::new().install_default_stylesheet(&mut runtime);
    let control = AppControl::new();
    crate::ui::app::window_theme::apply(&control, Default::default());
    control.apply(&mut runtime, &mut CssPipeline::new());
    runtime.set_root_view(move || dashboard.view(Page::Platform, None, false, false));
    runtime.run_due_frame_reactive().unwrap();

    let resolve = |runtime: &UiRuntime, name| {
        SelectorResolver::new()
            .resolve(runtime, &SelectorTarget::test_id(name))
            .unwrap()
    };
    let nav = runtime
        .inspect_snapshot()
        .nodes
        .into_iter()
        .find(|node| node.classes.iter().any(|class| class == "navigation"))
        .unwrap()
        .node;
    let policies = resolve(&runtime, "policies-navigation");
    runtime.scroll_into_view(policies);
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();
    assert!(runtime.scroll_top(nav) > 0.0);

    let semantics = runtime.semantic_snapshot();
    for id in ["face-navigation", "iris-navigation", "voice-navigation"] {
        let node = resolve(&runtime, id);
        assert!(semantics.get(node).unwrap().state().disabled);
        assert_eq!(runtime.computed_style(node).unwrap().opacity(), Some(1.0));
    }
}

#[test]
fn clicking_navigation_selects_fingerprints_and_policies() {
    use std::cell::Cell;
    use std::rc::Rc;

    use obscur_kit::bootstrap::{CssPipeline, KitBootstrap};
    use obscur_kit::control::AppControl;
    use obscur_kit::geometry::{Point, Size};
    use obscur_kit::headless::input::{SelectorResolver, SelectorTarget};
    use obscur_kit::identity::RuntimeId;
    use obscur_kit::interaction::pointer::{
        PointerDevice, PointerEvent, PointerEventKind, PointerId,
    };
    use obscur_kit::runtime::{UiRuntime, frame::FrameReason};

    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    let page = Rc::new(Cell::new(Page::Platform));
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(780.0, 580.0));
    KitBootstrap::new()
        .install_builtin_components(&mut runtime)
        .unwrap();
    let control = AppControl::new();
    crate::ui::widgets::WidgetImages::install(&control);
    crate::ui::app::window_theme::apply(&control, Default::default());
    control.apply(&mut runtime, &mut CssPipeline::new());
    let selected = Rc::clone(&page);
    runtime.on_command(Page::Fingerprints.command(), move || {
        selected.set(Page::Fingerprints)
    });
    let selected = Rc::clone(&page);
    runtime.on_command(Page::Policies.command(), move || {
        selected.set(Page::Policies)
    });
    let rendered = Rc::clone(&page);
    runtime.set_root_view(move || dashboard.view(rendered.get(), None, false, false));
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();

    runtime.poll_resource_tasks();
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();
    assert_eq!(runtime.cached_resource_counts(), (0, 8));

    let nodes = runtime.inspect_snapshot().nodes;
    let [sidebar_title, header, content_pane, title] =
        ["sidebar-title", "header", "content-pane", "title"].map(|class| {
            let node = nodes
                .iter()
                .find(|node| node.classes.iter().any(|name| name == class))
                .unwrap();
            runtime.layout_boxes().get(&node.node).unwrap()
        });
    assert_eq!(sidebar_title.origin().y, 0.0);
    assert_eq!(header.origin().y, 0.0);
    assert_eq!(sidebar_title.size().height, header.size().height);

    let center =
        |area: &obscur_kit::layout::model::LayoutBox| area.origin().x + area.size().width / 2.0;
    assert!(
        (center(content_pane) - center(title)).abs() < 0.5,
        "pane center {}, title center {}",
        center(content_pane),
        center(title)
    );

    let footer = nodes
        .iter()
        .find(|node| {
            node.classes
                .iter()
                .any(|name| name == "sidebar-footer-content")
        })
        .unwrap();
    let footer = runtime.layout_boxes().get(&footer.node).unwrap();
    assert_eq!(footer.origin().y + footer.size().height, 556.0);
    assert_eq!(footer.origin().x, 22.0);
    assert_eq!(footer.origin().x + footer.size().width, 226.0);

    let background = |runtime: &UiRuntime, node| {
        runtime
            .computed_style(node)
            .unwrap()
            .get(obscur_kit::style::property::PropertyId::BackgroundColor)
            .and_then(obscur_kit::style::computed::ComputedValue::as_color)
            .unwrap()
    };
    let hover_overlay = |runtime: &UiRuntime, node| {
        runtime
            .computed_style(node)
            .unwrap()
            .get(obscur_kit::style::property::PropertyId::BackgroundImage)
            .and_then(obscur_kit::style::computed::ComputedValue::as_linear_gradient)
            .is_some()
    };
    for (test_id, expected) in [
        ("fingerprints-navigation", Page::Fingerprints),
        ("policies-navigation", Page::Policies),
    ] {
        let button = SelectorResolver::new()
            .resolve(&runtime, &SelectorTarget::test_id(test_id))
            .unwrap();
        let layout = runtime.layout_boxes().get(&button).unwrap();
        let click = Point::new(
            layout.origin().x + layout.size().width / 2.0,
            layout.origin().y + layout.size().height / 2.0,
        );
        let resting = background(&runtime, button);
        runtime.handle_pointer(
            PointerEvent::new(
                PointerId::MOUSE,
                PointerDevice::Mouse,
                PointerEventKind::Move,
                click,
            )
            .as_trusted(),
        );
        runtime.run_due_frame_reactive().unwrap();
        assert_eq!(background(&runtime, button), resting);
        assert!(hover_overlay(&runtime, button));
        for kind in [PointerEventKind::Down, PointerEventKind::Up] {
            runtime.handle_pointer(
                PointerEvent::new(PointerId::MOUSE, PointerDevice::Mouse, kind, click).as_trusted(),
            );
        }
        assert!(page.get() == expected);
        runtime.request_frame(FrameReason::UiCommit);
        runtime.run_due_frame_reactive().unwrap();
        let selected_hover = background(&runtime, button);
        assert_ne!(selected_hover, resting);
        assert!(hover_overlay(&runtime, button));
        runtime.handle_pointer(
            PointerEvent::new(
                PointerId::MOUSE,
                PointerDevice::Mouse,
                PointerEventKind::Move,
                Point::new(50.0, 30.0),
            )
            .as_trusted(),
        );
        runtime.run_due_frame_reactive().unwrap();
        assert_eq!(background(&runtime, button), selected_hover);
        assert!(!hover_overlay(&runtime, button));
    }
}

#[test]
fn fingerprints_page_replaces_entries_when_access_is_denied() {
    let localizer = Localizer::system(carosign_l10n::Preferences::new("en", None));
    let dashboard = Dashboard::new(localizer).unwrap();
    let snapshot = Snapshot {
        health: Err(QueryError::Unavailable),
        inventory: Ok(Authenticators::default()),
        enrollments: Ok(vec![EnrollmentSummary {
            label: "Test fingerprint".into(),
            pending_reconciliation: true,
            ..EnrollmentSummary::default()
        }]),
    };

    let view = dashboard.view(Page::Fingerprints, Some(&snapshot), false, false);
    assert!(!has_outlet(view.root().nodes()));
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&"Test fingerprint"));
    assert!(!text.contains(&"Reconciliation required"));
    assert!(text.contains(&"Current account"));

    let snapshot = Snapshot {
        enrollments: Err(QueryError::Unauthorized),
        ..snapshot
    };
    let view = dashboard.view(Page::Fingerprints, Some(&snapshot), false, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(!text.contains(&"Test fingerprint"));
    assert!(text.contains(&"No access to fingerprints"));

    let unavailable_list = Snapshot {
        health: Ok(Health {
            service_ready: true,
            ..Health::default()
        }),
        enrollments: Err(QueryError::Unavailable),
        ..snapshot.clone()
    };
    let view = dashboard.view(Page::Fingerprints, Some(&unavailable_list), true, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&"Could not load fingerprints"));

    let snapshot = Snapshot {
        enrollments: Ok(Vec::new()),
        ..snapshot
    };
    let view = dashboard.view(Page::Fingerprints, Some(&snapshot), true, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&"No registered fingerprints"));

    let view = dashboard.view(Page::Fingerprints, None, true, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&"Loading fingerprints…"));
}

#[test]
fn fingerprint_error_text_fits_its_card_when_the_window_is_wide() {
    use obscur_kit::{
        AppControl, bootstrap::CssPipeline, geometry::Size, identity::RuntimeId, runtime::UiRuntime,
    };

    for width in [480.0, 600.0, 752.0, 1096.0] {
        let localizer = Localizer::system(carosign_l10n::Preferences::new("ru", None));
        let dashboard = Dashboard::new(localizer).unwrap();
        let snapshot = Snapshot {
            health: Ok(Health {
                service_ready: true,
                ..Health::default()
            }),
            inventory: Ok(Authenticators::default()),
            enrollments: Err(QueryError::Unavailable),
        };
        let control = AppControl::new();
        let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(width, 674.0));
        crate::ui::app::window_theme::apply(&control, Default::default());
        control.apply(&mut runtime, &mut CssPipeline::new());
        runtime.set_root_view(move || {
            dashboard.view(Page::Fingerprints, Some(&snapshot), width < 700.0, false)
        });
        runtime.run_due_frame_reactive().unwrap();

        let nodes = runtime.inspect_snapshot().nodes;
        let bounds = |class: &str| {
            let node = nodes
                .iter()
                .find(|node| node.classes.iter().any(|name| name == class))
                .unwrap();
            *runtime.layout_boxes().get(&node.node).unwrap()
        };
        let card = bounds("fingerprint-message");
        assert!(card.size().width <= 480.0);
        for class in ["fingerprint-message-title", "value"] {
            let text = bounds(class);
            assert!(
                text.origin().x >= card.origin().x + 23.0,
                "{class} at {width}"
            );
            assert!(
                text.origin().x + text.size().width <= card.origin().x + card.size().width - 23.0,
                "{class} at {width}"
            );
        }
    }
}

#[test]
fn policies_page_validates_the_current_draft_with_the_service_parser() {
    let localizer = Localizer::system(carosign_l10n::Preferences::new("en", None));
    let dashboard = Dashboard::new(localizer).unwrap();
    assert!(matches!(
        dashboard.policies().command("policy_unlock"),
        Some(crate::ui::widgets::PolicyJob::Read)
    ));
    assert!(dashboard.policies().command("policy_unlock").is_none());
    dashboard
        .policies()
        .finish(Ok(crate::ui::widgets::PolicyResult::Loaded {
            document: "uid=1000 subject=42 actions=verify purpose=session-login lifetime=30".into(),
            accounts: Some(vec![
                SystemAccount {
                    name: "alice".into(),
                    uid: 1000,
                    avatar: None,
                },
                SystemAccount {
                    name: "bob".into(),
                    uid: 42,
                    avatar: None,
                },
            ]),
        }));
    assert!(dashboard.policies().command("policy_write").is_none());
    let view = dashboard.view(Page::Policies, None, false, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(!text.contains(&"There are unsaved changes."));

    assert!(text.contains(&"alice (1000)"));
    assert!(text.contains(&"bob (42)"));

    dashboard.policies().command("policy_subject");
    dashboard.policies().edit("select_search", "alice");
    let view = dashboard.view(Page::Policies, None, false, false);
    assert!(!has_outlet(view.root().nodes()));
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert_eq!(
        text.iter()
            .filter(|value| **value == "alice (1000)")
            .count(),
        2
    );
    assert!(dashboard.policies().command("policy_unlock").is_none());
    dashboard.policies().command("policy_account_0");
    let view = dashboard.view(Page::Policies, None, false, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(!text.contains(&"bob (42)"));

    dashboard.policies().edit("lifetime", "wrong");
    let view = dashboard.view(Page::Policies, None, false, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&"There are unsaved changes."));

    assert!(dashboard.policies().command("policy_write").is_none());
    let view = dashboard.view(Page::Policies, None, false, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&"The rules failed validation."));
}

#[test]
fn policies_lists_rules_and_selects_the_clicked_row_for_editing() {
    let localizer = Localizer::system(carosign_l10n::Preferences::new("en", None));
    let inventory_label = localizer.text("ui-policy-inventory");
    let verify_label = localizer.text("ui-policy-verify");
    let dashboard = Dashboard::new(localizer).unwrap();
    dashboard
        .policies()
        .finish(Ok(crate::ui::widgets::PolicyResult::Loaded {
            document:
                "uid=0 subject=1000 actions=inventory\nuid=1000 subject=1000 actions=verify\n"
                    .into(),
            accounts: Some(vec![
                SystemAccount {
                    name: "alice".into(),
                    uid: 1000,
                    avatar: None,
                },
                SystemAccount {
                    name: "root".into(),
                    uid: 0,
                    avatar: None,
                },
            ]),
        }));
    let view = dashboard.view(Page::Policies, None, false, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&format!("root (0) → alice (1000) · {inventory_label}").as_str()));
    assert!(text.contains(&format!("alice (1000) → alice (1000) · {verify_label}").as_str()));
    assert!(text.contains(&"root (0)"));

    dashboard.policies().command("policy_rule_1");
    let view = dashboard.view(Page::Policies, None, false, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&format!("root (0) → alice (1000) · {inventory_label}").as_str()));
    assert!(!text.contains(&"root (0)"));
}

#[test]
fn account_selector_preserves_layout_and_dismisses_on_outside_press() {
    use std::rc::Rc;

    use obscur_kit::AppControl;
    use obscur_kit::bootstrap::{CssPipeline, KitBootstrap};
    use obscur_kit::geometry::{Point, Size};
    use obscur_kit::identity::{InternedSymbol, RuntimeId};
    use obscur_kit::interaction::pointer::{
        PointerDevice, PointerEvent, PointerEventKind, PointerId,
    };
    use obscur_kit::runtime::{UiRuntime, frame::FrameReason};

    let dashboard = Rc::new(Dashboard::new(Localizer::from_environment()).unwrap());
    dashboard
        .policies()
        .finish(Ok(crate::ui::widgets::PolicyResult::Loaded {
            document: "uid=1000 subject=1000 actions=verify\n".into(),
            accounts: Some(
                (0..9)
                    .map(|index| SystemAccount {
                        name: format!("user-{index}"),
                        uid: 1000 + index,
                        avatar: None,
                    })
                    .collect(),
            ),
        }));
    dashboard.policies().command("policy_rule_0");
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(780.0, 580.0));
    KitBootstrap::new()
        .install_builtin_components(&mut runtime)
        .unwrap();
    let control = AppControl::new();
    crate::ui::widgets::WidgetImages::install(&control);
    crate::ui::app::window_theme::apply(&control, Default::default());
    control.apply(&mut runtime, &mut CssPipeline::new());
    let selected = Rc::clone(&dashboard);
    runtime.on_command(InternedSymbol::named("policy_account_0"), move || {
        selected.policies().command("policy_account_0");
    });
    for command in ["navigation_swipe", "window_move"] {
        let menu = Rc::clone(&dashboard);
        runtime.on_command(InternedSymbol::named(command), move || {
            menu.policies().dismiss_account_menu();
        });
    }
    let rendered = Rc::clone(&dashboard);
    runtime.set_root_view(move || rendered.view(Page::Policies, None, false, false));
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();

    runtime.poll_resource_tasks();
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();
    assert_eq!(runtime.cached_resource_counts(), (0, 8));
    let nodes = runtime.inspect_snapshot().nodes;
    let [header, back] = ["header", "header-back"].map(|class| {
        let node = nodes
            .iter()
            .find(|node| node.classes.iter().any(|name| name == class))
            .unwrap();
        runtime.layout_boxes().get(&node.node).unwrap()
    });
    assert!(
        ((header.origin().y + header.size().height / 2.0)
            - (back.origin().y + back.size().height / 2.0))
            .abs()
            < 1.0
    );
    assert!(
        ((back.origin().x - header.origin().x) - (back.origin().y - header.origin().y)).abs() < 1.0
    );

    let y = |runtime: &UiRuntime, class: &str| {
        let node = runtime
            .inspect_snapshot()
            .nodes
            .into_iter()
            .find(|node| node.classes.iter().any(|name| name == class))
            .unwrap();
        runtime.layout_boxes().get(&node.node).unwrap().origin().y
    };
    let actions = y(&runtime, "policy-option-group");

    dashboard.policies().command("policy_caller");
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();

    assert!(y(&runtime, "select-menu") > y(&runtime, "select-trigger"));
    assert_eq!(y(&runtime, "policy-option-group"), actions);

    crate::ui::app::account_menu_scroll::reveal(&mut runtime, "policy-caller-trigger");
    let popup = runtime
        .inspect_snapshot()
        .nodes
        .into_iter()
        .find(|node| node.classes.iter().any(|class| class == "select-menu"))
        .unwrap();
    let popup_box = runtime.layout_boxes().get(&popup.node).unwrap();
    assert!(popup_box.size().height >= 72.0);
    assert!(popup_box.origin().y + popup_box.size().height <= runtime.viewport().height);
    let row = runtime
        .inspect_snapshot()
        .nodes
        .into_iter()
        .find(|node| {
            node.classes
                .iter()
                .any(|class| class == "select-menu-option")
        })
        .unwrap();
    let origin = runtime.layout_boxes().get(&row.node).unwrap().origin();
    let point = Point::new(origin.x + 12.0, origin.y + 12.0);
    let scroller = runtime
        .inspect_snapshot()
        .nodes
        .into_iter()
        .find(|node| node.classes.iter().any(|class| class == "select-options"))
        .unwrap();
    assert_eq!(runtime.handle_scroll(120.0, point), Some(scroller.node));
    assert!(runtime.scroll_top(scroller.node) > 0.0);
    runtime.set_scroll_top(scroller.node, 0.0);
    for kind in [PointerEventKind::Down, PointerEventKind::Up] {
        runtime.handle_pointer(
            PointerEvent::new(PointerId::MOUSE, PointerDevice::Mouse, kind, point).as_trusted(),
        );
        if kind == PointerEventKind::Down {
            assert!(dashboard.policies().account_menu_trigger().is_some());
        }
    }
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();
    assert!(
        !runtime
            .inspect_snapshot()
            .nodes
            .iter()
            .any(|node| { node.classes.iter().any(|class| class == "select-menu") })
    );

    dashboard.policies().command("policy_caller");
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();
    runtime.handle_pointer(
        PointerEvent::new(
            PointerId::MOUSE,
            PointerDevice::Mouse,
            PointerEventKind::Down,
            Point::new(50.0, 30.0),
        )
        .as_trusted(),
    );
    assert!(dashboard.policies().account_menu_trigger().is_none());
}

#[test]
fn policy_deletion_can_be_undone_and_conflict_preserves_the_draft() {
    use crate::ui::widgets::{PolicyJob, PolicyResult};

    let localizer = Localizer::system(carosign_l10n::Preferences::new("en", None));
    let inventory_label = localizer.text("ui-policy-inventory");
    let dashboard = Dashboard::new(localizer).unwrap();
    let policies = dashboard.policies();
    policies.finish(Err("ui-policy-unavailable"));
    assert!(matches!(
        policies.command("policy_unlock"),
        Some(PolicyJob::Read)
    ));

    let document = "uid=0 subject=1000 actions=inventory\n";
    policies.finish(Ok(PolicyResult::Loaded {
        document: document.into(),
        accounts: None,
    }));
    policies.command("policy_rule_0");
    policies.command("policy_delete");
    policies.command("policy_undo_delete");
    assert!(policies.command("policy_write").is_none());

    policies.command("policy_rule_0");
    policies.command("policy_delete");
    assert!(matches!(
        policies.command("policy_write"),
        Some(PolicyJob::Write { .. })
    ));
    policies.finish(Err("ui-policy-conflict"));
    assert!(policies.command("policy_unlock").is_none());
    assert!(matches!(
        policies.command("policy_reload"),
        Some(PolicyJob::Read)
    ));
    policies.finish(Err("ui-policy-denied"));
    policies.command("policy_undo_delete");
    let view = dashboard.view(Page::Policies, None, false, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&format!("UID 0 → UID 1000 · {inventory_label}").as_str()));
}

#[test]
fn policy_choices_keep_indicators_and_labels_within_their_columns() {
    use obscur_kit::AppControl;
    use obscur_kit::bootstrap::{CssPipeline, KitBootstrap};
    use obscur_kit::geometry::Size;
    use obscur_kit::identity::RuntimeId;
    use obscur_kit::runtime::{UiRuntime, frame::FrameReason};

    let dashboard = Dashboard::new(Localizer::system(carosign_l10n::Preferences::new(
        "ru", None,
    )))
    .unwrap();
    dashboard
        .policies()
        .finish(Ok(crate::ui::widgets::PolicyResult::Loaded {
            document: "uid=1000 subject=1000 actions=verify purpose=session-login\n".into(),
            accounts: None,
        }));
    dashboard.policies().command("policy_rule_0");
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(780.0, 580.0));
    KitBootstrap::new()
        .install_builtin_components(&mut runtime)
        .unwrap();
    KitBootstrap::new().install_default_stylesheet(&mut runtime);
    let control = AppControl::new();
    crate::ui::app::window_theme::apply(&control, Default::default());
    control.apply(&mut runtime, &mut CssPipeline::new());
    runtime.set_root_view(move || dashboard.view(Page::Policies, None, false, false));
    for width in [900.0, 780.0, 600.0, 480.0, 400.0] {
        runtime.set_viewport(Size::new(width, 580.0));
        runtime.request_frame(FrameReason::UiCommit);
        runtime.run_due_frame_reactive().unwrap();
        let snapshot = runtime.inspect_snapshot();
        let mut count = 0;
        for node in &snapshot.nodes {
            if !node
                .classes
                .iter()
                .any(|class| matches!(class.as_str(), "checkbox-box" | "radio-dot"))
            {
                continue;
            }
            let Some(bounds) = node.bounds else {
                continue;
            };
            assert_eq!(
                bounds.size,
                Size::new(16.0, 16.0),
                "window {width}px, {:?}",
                node.classes
            );
            assert!(
                bounds.origin.x + bounds.size.width <= width,
                "window {width}px, {bounds:?}"
            );
            count += 1;
        }
        let choices: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| node.classes.iter().any(|class| class == "policy-choice"))
            .collect();
        for pair in choices.windows(2) {
            let bounds = pair[0].bounds.unwrap();
            let next = pair[1].bounds.unwrap();
            if (bounds.origin.y - next.origin.y).abs() > 1.0 || next.origin.x <= bounds.origin.x {
                continue;
            }
            let label = snapshot
                .nodes
                .iter()
                .find(|node| node.parent == Some(pair[0].node) && node.text.is_some())
                .unwrap();
            let label = label.bounds.unwrap();
            assert!(
                label.origin.x + label.size.width <= next.origin.x,
                "window {width}px: label {label:?} overlaps next choice {next:?}"
            );
        }
        assert_eq!(count, 12);
    }
}

#[test]
fn adaptive_transition_moves_the_content_edge_through_intermediate_layouts() {
    use std::{cell::Cell, rc::Rc};

    use obscur_kit::AppControl;
    use obscur_kit::bootstrap::CssPipeline;
    use obscur_kit::geometry::Size;
    use obscur_kit::identity::RuntimeId;
    use obscur_kit::runtime::{UiRuntime, frame::FrameReason};

    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    let compact = Rc::new(Cell::new(false));
    let rendered = Rc::clone(&compact);
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(780.0, 580.0));
    let control = AppControl::new();
    crate::ui::app::window_theme::apply(&control, Default::default());
    let mut css = CssPipeline::new();
    control.apply(&mut runtime, &mut css);
    runtime.set_root_view(move || dashboard.view(Page::Platform, None, rendered.get(), true));
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();

    let content_left = |runtime: &UiRuntime| {
        let pane = runtime
            .inspect_snapshot()
            .nodes
            .into_iter()
            .find(|node| node.classes.iter().any(|class| class == "content-pane"))
            .unwrap();
        runtime.layout_boxes().get(&pane.node).unwrap().origin().x
    };
    let wide = content_left(&runtime);
    assert_eq!(wide, 248.0);
    compact.set(true);
    runtime.set_viewport(Size::new(680.0, 580.0));
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();
    assert_eq!(content_left(&runtime), wide);

    control.apply(&mut runtime, &mut css);
    let mut previous = wide;
    for _ in 0..8 {
        assert!(runtime.has_pending_frame());
        runtime.advance_motion(0.016);
        control.apply(&mut runtime, &mut css);
        runtime.run_due_frame_reactive().unwrap();
        control.apply(&mut runtime, &mut css);
        let intermediate = content_left(&runtime);
        assert!(
            intermediate > 0.0 && intermediate < previous,
            "intermediate edge {intermediate}"
        );
        previous = intermediate;
    }

    runtime.advance_motion(0.25);
    control.apply(&mut runtime, &mut css);
    runtime.run_due_frame_reactive().unwrap();
    assert_eq!(content_left(&runtime), 0.0);

    compact.set(false);
    runtime.set_viewport(Size::new(780.0, 580.0));
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();
    assert_eq!(content_left(&runtime), 0.0);
    runtime.advance_motion(0.10);
    runtime.run_due_frame_reactive().unwrap();
    assert!(content_left(&runtime) > 0.0 && content_left(&runtime) < wide);
}

#[test]
fn policy_unlock_gates_the_editor_and_allows_retry_after_denial() {
    use crate::ui::widgets::{PolicyJob, PolicyResult};

    let localizer = Localizer::system(carosign_l10n::Preferences::new("en", None));
    let dashboard = Dashboard::new(Localizer::system(carosign_l10n::Preferences::new(
        "en", None,
    )))
    .unwrap();
    let policies = dashboard.policies();
    let visible_text = |key: &str| {
        let view = dashboard.view(Page::Policies, None, true, false);
        let mut text = Vec::new();
        collect_text(view.root(), &mut text);
        let label = localizer.text(key);
        text.contains(&label.as_str())
    };

    assert!(visible_text("ui-policy-access-title"));
    assert!(!visible_text("ui-policy-save"));
    assert!(policies.command("policy_add").is_none());
    assert!(policies.command("policy_reload").is_none());

    assert!(matches!(
        policies.command("policy_unlock"),
        Some(PolicyJob::Read)
    ));
    assert!(visible_text("ui-policy-access-pending"));
    assert!(policies.command("policy_unlock").is_none());

    policies.finish(Err("ui-policy-denied"));
    assert!(visible_text("ui-policy-access-denied"));
    assert!(matches!(
        policies.command("policy_unlock"),
        Some(PolicyJob::Read)
    ));
    policies.finish(Ok(PolicyResult::Loaded {
        document: String::new(),
        accounts: None,
    }));
    assert!(!visible_text("ui-policy-access-title"));
    assert!(visible_text("ui-policy-save"));
}

#[test]
fn locked_policy_panel_is_centered_in_the_content_area() {
    use obscur_kit::{
        AppControl,
        bootstrap::CssPipeline,
        geometry::Size,
        identity::RuntimeId,
        runtime::{UiRuntime, frame::FrameReason},
    };
    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    for (width, compact) in [(840.0, false), (540.0, true)] {
        let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(width, 580.0));
        let control = AppControl::new();
        crate::ui::app::window_theme::apply(&control, Default::default());
        control.apply(&mut runtime, &mut CssPipeline::new());
        let view = dashboard.view(Page::Policies, None, compact, false);
        runtime.set_root_view(move || view.clone());
        runtime.request_frame(FrameReason::UiCommit);
        runtime.run_due_frame_reactive().unwrap();
        let nodes = runtime.inspect_snapshot().nodes;
        let [area, access] = ["content-centered", "policy-access"].map(|class| {
            let node = nodes
                .iter()
                .find(|node| node.classes.iter().any(|name| name == class))
                .unwrap();
            runtime.layout_boxes().get(&node.node).unwrap()
        });
        assert!(
            (area.origin().x + area.size().width / 2.0
                - access.origin().x
                - access.size().width / 2.0)
                .abs()
                < 1.0
        );
        assert!(
            (area.origin().y + area.size().height / 2.0
                - access.origin().y
                - access.size().height / 2.0)
                .abs()
                < 1.0
        );
    }
}

#[test]
fn service_restart_is_available_while_policies_are_locked_and_can_be_retried() {
    use crate::ui::widgets::{PolicyJob, PolicyResult};

    let dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    let policies = dashboard.policies();
    assert!(policies.requires_unlock());
    assert!(matches!(
        policies.command("policy_restart_service"),
        Some(PolicyJob::RestartService)
    ));
    assert!(policies.command("policy_restart_service").is_none());
    policies.finish(Err("ui-policy-restart-failed"));
    assert!(matches!(
        policies.command("policy_restart_service"),
        Some(PolicyJob::RestartService)
    ));
    policies.finish(Ok(PolicyResult::ServiceRestarted));
    assert!(policies.requires_unlock());
}

#[test]
fn fingerprint_demo_uses_the_regular_list_even_when_the_service_is_unavailable() {
    let localizer = Localizer::system(carosign_l10n::Preferences::new("en", None));
    let mut dashboard = Dashboard::new(localizer).unwrap();
    dashboard.enable_fingerprint_demo();
    let snapshot = Snapshot {
        health: Err(QueryError::Unavailable),
        inventory: Err(QueryError::Unavailable),
        enrollments: Err(QueryError::Unavailable),
    };
    let view = dashboard.view(Page::Fingerprints, Some(&snapshot), true, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&"Right index finger"));
    assert!(text.contains(&"Left thumb"));
    assert!(text.contains(&"Right thumb"));
    assert!(text.contains(&"Add fingerprint"));
    assert!(dashboard.fingerprint_command("fingerprint_add"));
    assert!(dashboard.is_editor(Page::Fingerprints));
    let view = dashboard.view(Page::Fingerprints, Some(&snapshot), true, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&"Fingerprint enrollment"));
    assert!(text.contains(&"0 / 20"));
    assert!(text.contains(&"Place your finger on the sensor."));
    assert!(!text.contains(&"Save fingerprint"));
    assert!(!text.contains(&"Delete fingerprint"));
    assert!(!dashboard.fingerprint_command("fingerprint_discard"));
    for accepted in 1..=20 {
        assert!(dashboard.fingerprint_command("fingerprint_next"));
        let view = dashboard.view(Page::Fingerprints, Some(&snapshot), true, false);
        let mut text = Vec::new();
        collect_text(view.root(), &mut text);
        if accepted == 20 {
            assert!(text.contains(&"Scanning complete"));
            assert!(text.contains(&"Save fingerprint"));
            assert!(text.contains(&"Delete fingerprint"));
        } else {
            let count = format!("{accepted} / 20");
            let hint = match accepted % 3 {
                1 => "Shift your finger slightly to capture another area.",
                2 => "Center your finger on the sensor.",
                _ => "Place your finger on the sensor.",
            };
            assert!(text.contains(&count.as_str()));
            assert!(text.contains(&hint));
            assert!(!text.contains(&"Save fingerprint"));
            assert!(!text.contains(&"Delete fingerprint"));
        }
    }
    assert!(dashboard.fingerprint_command("fingerprint_next"));
    assert!(!dashboard.is_editor(Page::Fingerprints));
    let view = dashboard.view(Page::Fingerprints, Some(&snapshot), true, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&"Demo fingerprint 4"));

    assert!(!text.contains(&"Reconciliation required"));

    assert!(!text.contains(&"Delete fingerprint"));
    assert!(dashboard.fingerprint_command("fingerprint_add"));
    for _ in 0..20 {
        assert!(dashboard.fingerprint_command("fingerprint_next"));
    }
    assert!(dashboard.fingerprint_command("fingerprint_discard"));
    assert!(!dashboard.is_editor(Page::Fingerprints));
    let view = dashboard.view(Page::Fingerprints, Some(&snapshot), true, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&"Demo fingerprint 4"));
    assert!(!text.contains(&"Demo fingerprint 5"));
    assert!(!text.contains(&"Delete fingerprint"));
}

#[test]
fn registered_fingerprint_actions_follow_the_selected_row() {
    use obscur_kit::{
        AppControl,
        bootstrap::CssPipeline,
        geometry::{Point, Size},
        headless::input::{SelectorResolver, SelectorTarget},
        identity::{InternedSymbol, RuntimeId},
        interaction::{
            event::UiEvent,
            pointer::{PointerDevice, PointerEvent, PointerEventKind, PointerId},
        },
        runtime::{UiRuntime, frame::FrameReason},
    };
    use std::rc::Rc;

    let localizer = Localizer::system(carosign_l10n::Preferences::new("en", None));
    let mut dashboard = Dashboard::new(localizer).unwrap();
    dashboard.enable_fingerprint_demo();
    let dashboard = Rc::new(dashboard);
    let rendered = Rc::clone(&dashboard);
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(320.0, 580.0));
    let control = AppControl::new();
    let mut css = CssPipeline::new();
    crate::ui::app::window_theme::apply(&control, Default::default());
    control.apply(&mut runtime, &mut css);
    let selected = Rc::clone(&dashboard);
    let select_control = control.clone();
    runtime.on_command(
        InternedSymbol::named("fingerprint_select"),
        move |event: &mut UiEvent| {
            let target = event.target.unwrap();
            let selected = Rc::clone(&selected);
            select_control.with_runtime(move |runtime| {
                assert!(selected.select_registered_fingerprint(runtime, target));
            });
        },
    );
    runtime.set_root_view(move || rendered.view(Page::Fingerprints, None, true, false));
    let render = |runtime: &mut UiRuntime| {
        runtime.request_frame(FrameReason::UiCommit);
        runtime.run_due_frame_reactive().unwrap();
    };
    render(&mut runtime);
    let resolver = SelectorResolver::new();
    let find_row = |runtime: &UiRuntime, id| {
        resolver.resolve(
            runtime,
            &SelectorTarget::test_id(format!("fingerprint-row-{id}")),
        )
    };
    let row = find_row(&runtime, 2).unwrap();
    let bounds = runtime.layout_boxes().get(&row).unwrap();
    let point = Point::new(bounds.origin().x + 20.0, bounds.origin().y + 20.0);
    for kind in [PointerEventKind::Down, PointerEventKind::Up] {
        runtime.handle_pointer(
            PointerEvent::new(PointerId::MOUSE, PointerDevice::Mouse, kind, point).as_trusted(),
        );
    }
    control.apply(&mut runtime, &mut css);
    render(&mut runtime);
    let actions: Vec<_> = runtime
        .inspect_snapshot()
        .nodes
        .into_iter()
        .filter(|node| {
            node.classes
                .iter()
                .any(|class| class == "fingerprint-row-actions")
        })
        .collect();
    assert_eq!(actions.len(), 1);
    let bounds = runtime.layout_boxes();
    let selected = bounds.get(&row).unwrap();
    let actions = bounds.get(&actions[0].node).unwrap();
    let following = bounds.get(&find_row(&runtime, 3).unwrap()).unwrap();
    assert!(selected.origin().y + selected.size().height <= actions.origin().y);
    assert!(actions.origin().y + actions.size().height <= following.origin().y);
    use obscur_kit::style::{
        computed::ComputedValue,
        property::{Length, PropertyId},
    };
    let assert_radius = |runtime: &UiRuntime, node, radius| {
        let style = runtime.computed_style(node).unwrap();
        for corner in [
            PropertyId::BorderTopLeftRadius,
            PropertyId::BorderTopRightRadius,
            PropertyId::BorderBottomRightRadius,
            PropertyId::BorderBottomLeftRadius,
        ] {
            assert_eq!(
                style.get(corner).and_then(ComputedValue::as_length),
                Some(Length::px(radius))
            );
        }
    };
    let list = runtime
        .inspect_snapshot()
        .nodes
        .into_iter()
        .find(|node| node.classes.iter().any(|class| class == "fingerprint-rows"))
        .unwrap();
    assert_radius(&runtime, list.node, 12.0);
    assert_radius(&runtime, row, 0.0);
    for id in ["fingerprint-rename", "fingerprint-delete"] {
        let button = resolver
            .resolve(&runtime, &SelectorTarget::test_id(id))
            .unwrap();
        assert_radius(&runtime, button, 7.0);
        let button = bounds.get(&button).unwrap();
        assert_eq!(button.size().height, 34.0);
        assert!(button.origin().x >= actions.origin().x);
        assert!(
            button.origin().x + button.size().width <= actions.origin().x + actions.size().width
        );
    }

    let row_top = selected.origin().y;
    let row_bottom = row_top + selected.size().height;
    assert!(dashboard.fingerprint_command("fingerprint_rename"));
    render(&mut runtime);
    let input = resolver
        .resolve(
            &runtime,
            &SelectorTarget::test_id("fingerprint-inline-name"),
        )
        .unwrap();
    assert_radius(&runtime, input, 7.0);
    let input = runtime.layout_boxes().get(&input).unwrap();
    assert!(input.origin().y >= row_top);
    assert!(input.origin().y + input.size().height <= row_bottom);
    assert!(dashboard.edit_registered_fingerprint_name("  "));
    assert!(!dashboard.fingerprint_command("fingerprint_rename_save"));
    assert!(dashboard.edit_registered_fingerprint_name("  Left thumb renamed  "));
    assert!(dashboard.fingerprint_command("fingerprint_rename_save"));
    let view = dashboard.view(Page::Fingerprints, None, true, false);
    let mut text = Vec::new();
    collect_text(view.root(), &mut text);
    assert!(text.contains(&"Left thumb renamed"));
    assert!(!text.contains(&"Left thumb"));
    assert!(dashboard.fingerprint_command("fingerprint_delete"));
    render(&mut runtime);
    assert!(find_row(&runtime, 2).is_some());
    assert!(dashboard.fingerprint_command("fingerprint_delete_cancel"));
    assert!(dashboard.fingerprint_command("fingerprint_delete"));
    assert!(dashboard.fingerprint_command("fingerprint_delete_confirm"));
    render(&mut runtime);
    assert!(find_row(&runtime, 2).is_none());
    assert!(find_row(&runtime, 1).is_some());
    assert!(find_row(&runtime, 3).is_some());
}

#[test]
fn fingerprints_keep_aligned_rows_and_icons_at_both_widths() {
    use obscur_kit::{
        AppControl,
        bootstrap::CssPipeline,
        geometry::Size,
        identity::RuntimeId,
        runtime::{UiRuntime, frame::FrameReason},
    };

    let mut dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    dashboard.enable_fingerprint_demo();
    for (width, compact) in [(900.0, false), (320.0, true)] {
        let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(width, 580.0));
        let control = AppControl::new();
        crate::ui::app::window_theme::apply(&control, Default::default());
        control.apply(&mut runtime, &mut CssPipeline::new());
        let view = dashboard.view(Page::Fingerprints, None, compact, false);
        runtime.set_root_view(move || view.clone());
        runtime.request_frame(FrameReason::UiCommit);
        runtime.run_due_frame_reactive().unwrap();
        let rows: Vec<_> = runtime
            .inspect_snapshot()
            .nodes
            .into_iter()
            .filter(|node| node.classes.iter().any(|class| class == "fingerprint-row"))
            .map(|node| runtime.layout_boxes().get(&node.node).unwrap())
            .collect();
        assert_eq!(rows.len(), 3);
        let icons: Vec<_> = runtime
            .inspect_snapshot()
            .nodes
            .into_iter()
            .filter(|node| {
                node.classes
                    .iter()
                    .any(|class| class == "fingerprint-icon-area")
            })
            .map(|node| runtime.layout_boxes().get(&node.node).unwrap())
            .collect();
        assert_eq!(icons.len(), rows.len());
        for (row, icon) in rows.iter().zip(icons) {
            assert_eq!(icon.size().width, icon.size().height);
            let icon_center = icon.origin().y + icon.size().height / 2.0;
            let row_center = row.origin().y + row.size().height / 2.0;
            assert!((icon_center - row_center).abs() <= 0.5);
        }
        for pair in rows.windows(2) {
            assert_eq!(pair[0].origin().x, pair[1].origin().x);
            assert_eq!(pair[0].size().width, pair[1].size().width);
            assert!(pair[1].origin().y >= pair[0].origin().y + pair[0].size().height);
        }
    }
}

#[test]
fn enrollment_draws_along_ridges_across_twenty_demo_touches() {
    use obscur_kit::{
        AppControl,
        bootstrap::CssPipeline,
        geometry::Size,
        identity::RuntimeId,
        runtime::{UiRuntime, frame::FrameReason},
    };
    use std::rc::Rc;

    let mut dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    dashboard.enable_fingerprint_demo();
    assert!(dashboard.fingerprint_command("fingerprint_add"));
    let dashboard = Rc::new(dashboard);
    let rendered = Rc::clone(&dashboard);
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(540.0, 580.0));
    let control = AppControl::new();
    crate::ui::app::window_theme::apply(&control, Default::default());
    control.apply(&mut runtime, &mut CssPipeline::new());
    use crate::ui::widgets::{enrollment_print, enrollment_ring};
    use obscur_kit::scene::{
        command::PaintCommand,
        paint::{Brush, Paint},
        path::PathTrim,
    };

    let appearance = crate::ui::app::system_defaults::with_fallbacks(Default::default());
    let colors = crate::ui::app::window_theme::enrollment_colors(&appearance);
    runtime.register_canvas(
        enrollment_ring::PAINTER,
        Box::new(move |region, input| enrollment_ring::paint(region, input, colors)),
    );
    let mut fingerprint = enrollment_print::EnrollmentPrint::default();
    runtime.register_canvas(
        enrollment_print::PAINTER,
        Box::new(move |region, input| fingerprint.paint(region, input, colors)),
    );
    runtime.set_root_view(move || rendered.view(Page::Fingerprints, None, true, true));
    let ridge_length = |runtime: &UiRuntime, highlighted: bool| {
        runtime
            .scene()
            .root()
            .commands()
            .iter()
            .filter_map(|command| match command {
                PaintCommand::DrawPath { path, paint: Paint::Stroke(stroke) }
                    if stroke.width() > 3.0
                        && matches!(stroke.brush(), Brush::Solid(brush) if brush.color() == if highlighted { colors.progress } else { colors.track }) =>
                {
                    Some(PathTrim::of(path).length())
                }
                _ => None,
            })
            .sum::<f32>()
    };

    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();
    let bounds = |runtime: &UiRuntime, class: &str| {
        let node = runtime
            .inspect_snapshot()
            .nodes
            .into_iter()
            .find(|node| node.classes.iter().any(|name| name == class))
            .unwrap();
        *runtime.layout_boxes().get(&node.node).unwrap()
    };
    let hint = bounds(&runtime, "enrollment-hint");
    let mark = bounds(&runtime, "enrollment-mark");
    assert!(hint.origin().y + hint.size().height <= mark.origin().y);
    let actions_top = bounds(&runtime, "enrollment-actions").origin().y;
    let total = ridge_length(&runtime, false);
    assert!(total > 0.0);
    assert_eq!(ridge_length(&runtime, true), 0.0);

    for accepted in 1..=20 {
        let previous = ridge_length(&runtime, true);
        assert!(dashboard.fingerprint_command("fingerprint_next"));
        runtime.request_frame(FrameReason::UiCommit);
        runtime.run_due_frame_reactive().unwrap();
        if accepted < 20 {
            assert_eq!(
                bounds(&runtime, "enrollment-actions").origin().y,
                actions_top
            );
        }
        assert!((ridge_length(&runtime, true) - previous).abs() < 0.02);

        runtime.advance_motion(0.08);
        runtime.request_frame(FrameReason::Animation);
        runtime.run_due_frame_reactive().unwrap();
        let intermediate = ridge_length(&runtime, true);
        assert!(intermediate > previous);
        assert!((intermediate + ridge_length(&runtime, false) - total).abs() < 0.02);

        runtime.advance_motion(0.12);
        runtime.request_frame(FrameReason::Animation);
        runtime.run_due_frame_reactive().unwrap();
        let expected = total * accepted as f32 / 20.0;
        assert!(intermediate < expected);
        assert!((ridge_length(&runtime, true) - expected).abs() < 0.02);
    }
    assert!(ridge_length(&runtime, false) < 0.02);
}

#[test]
fn enrollment_mark_stays_centered_when_the_form_scales() {
    use obscur_kit::{
        AppControl,
        bootstrap::CssPipeline,
        geometry::Size,
        identity::RuntimeId,
        runtime::{UiRuntime, frame::FrameReason},
    };
    use std::rc::Rc;

    let mut dashboard = Dashboard::new(Localizer::from_environment()).unwrap();
    dashboard.enable_fingerprint_demo();
    assert!(dashboard.fingerprint_command("fingerprint_add"));
    let dashboard = Rc::new(dashboard);
    for (width, height, compact) in [
        (840.0, 580.0, false),
        (540.0, 580.0, true),
        (540.0, 240.0, true),
    ] {
        let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), Size::new(width, height));
        let control = AppControl::new();
        crate::ui::app::window_theme::apply(&control, Default::default());
        control.apply(&mut runtime, &mut CssPipeline::new());
        let rendered = Rc::clone(&dashboard);
        runtime.set_root_view(move || rendered.view(Page::Fingerprints, None, compact, false));
        runtime.request_frame(FrameReason::UiCommit);
        runtime.run_due_frame_reactive().unwrap();

        let nodes = runtime.inspect_snapshot().nodes;
        let [area, form, mark] = [
            "content-centered",
            "fingerprint-enrollment",
            "enrollment-mark",
        ]
        .map(|class| {
            nodes
                .iter()
                .find(|node| node.classes.iter().any(|name| name == class))
                .unwrap()
                .node
        });
        let area = runtime.layout_boxes().get(&area).unwrap();
        let bounds = runtime.layout_boxes().get(&form).unwrap();
        let area_center = area.origin().y + area.size().height / 2.0;
        let form_center = bounds.origin().y + bounds.size().height / 2.0;
        let mark = runtime.layout_boxes().get(&mark).unwrap();
        let mark_center = mark.origin().y + mark.size().height / 2.0;
        let form_top = bounds.origin().y;
        let form_bottom = form_top + bounds.size().height;
        let available = area.size().height;
        dashboard.fit_fingerprint_enrollment(&runtime);
        runtime.request_frame(FrameReason::UiCommit);
        runtime.run_due_frame_reactive().unwrap();
        let transform = runtime
            .computed_style(form)
            .unwrap()
            .get(obscur_kit::style::property::PropertyId::Transform)
            .and_then(obscur_kit::style::computed::ComputedValue::as_transform);
        let matrix = transform.unwrap().matrix();
        let scale = matrix[0];
        let visual_y = |y| form_center + (y - form_center) * scale + matrix[5];
        assert!((area_center - visual_y(mark_center)).abs() < 1.0);
        assert!(visual_y(form_top) >= area_center - available / 2.0);
        assert!(visual_y(form_bottom) <= area_center + available / 2.0);
        if height < 300.0 {
            assert!(scale < 1.0);
        }
        runtime.set_scroll_top(form, 1000.0);
        assert_eq!(runtime.scroll_top(form), 0.0);
    }
}
