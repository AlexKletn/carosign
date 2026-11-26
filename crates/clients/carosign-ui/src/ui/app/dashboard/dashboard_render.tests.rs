use std::{cell::Cell, path::Path, rc::Rc};

use carosign_l10n::Localizer;
use carosign_protocol::carosign::{
    client::v1::{Authenticators, Health, ProviderInventory, ProviderPublication},
    common::v1::{AuthenticatorDescriptor, UnavailabilityReason},
};
use obscur_kit::{
    AppControl,
    bootstrap::{CssPipeline, KitBootstrap},
    geometry::{Point, Size},
    headless::input::{SelectorResolver, SelectorTarget},
    identity::{InternedSymbol, RuntimeId},
    interaction::{
        command::{KeyEvent, LogicalKey, NamedKey, PhysicalKey},
        event::UiEvent,
        pointer::{PointerDevice, PointerEvent, PointerEventKind, PointerId},
    },
    render::frame::{FrameSubmission, RenderFrame},
    render_vello::renderer::{VelloDevice, VelloRenderer},
    runtime::{UiRuntime, frame::FrameReason},
    system::appearance::{ColorScheme, Contrast, SystemAppearance},
};

use super::Dashboard;
use crate::domain::{QueryError, Snapshot};
use crate::ui::{
    app::{
        account_menu_scroll, screen_transition::ScreenTransition, system_defaults, window_theme,
    },
    navigation::Navigation,
    page::Page,
    widgets::{PolicyJob, PolicyResult, WidgetImages, enrollment_print, enrollment_ring},
};

#[test]
#[ignore = "renders PNGs with a headless Vulkan device"]
fn render_dashboard() {
    let output = std::env::var_os("CAROSIGN_UI_SNAPSHOT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-snapshots"));
    std::fs::create_dir_all(&output).unwrap();

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: VelloDevice::diagnostic_features(&adapter),
        ..Default::default()
    }))
    .unwrap();
    let mut renderer = VelloRenderer::default().with_device(VelloDevice::lyon(device, queue));
    for (width, scheme, contrast, prefix) in [
        (480, ColorScheme::Dark, Contrast::Normal, "compact-dark"),
        (900, ColorScheme::Dark, Contrast::Normal, "wide-dark"),
        (1480, ColorScheme::Dark, Contrast::Normal, "large-dark"),
        (480, ColorScheme::Light, Contrast::Normal, "compact-light"),
        (900, ColorScheme::Light, Contrast::Normal, "wide-light"),
        (480, ColorScheme::Dark, Contrast::High, "compact-contrast"),
    ] {
        let appearance = SystemAppearance {
            color_scheme: Some(scheme),
            contrast: Some(contrast),
            text_scale: contrast.is_high().then_some(1.5),
            ..SystemAppearance::default()
        };
        render_states(&mut renderer, &output, width, appearance, prefix);
    }
}

fn render_states(
    renderer: &mut VelloRenderer,
    output: &Path,
    width: u32,
    appearance: SystemAppearance,
    prefix: &str,
) {
    let height = if width >= 900 { 600 } else { 460 };
    let size = Size::new(width as f32, height as f32);
    let bootstrap = KitBootstrap::new();
    let mut runtime = UiRuntime::new(RuntimeId::from_raw(1), size);
    bootstrap.install_default_stylesheet(&mut runtime);
    bootstrap.install_builtin_components(&mut runtime).unwrap();
    let fonts = bootstrap.install_text_stack(&mut runtime, &bootstrap.font_set());
    for (handle, bytes) in fonts.iter() {
        renderer.register_font(handle, bytes.to_vec());
    }

    let control = AppControl::new();
    let mut css = CssPipeline::new();
    WidgetImages::install(&control);
    let colors =
        window_theme::enrollment_colors(&system_defaults::with_fallbacks(appearance.clone()));
    let mut print = enrollment_print::EnrollmentPrint::default();
    runtime.register_canvas(
        enrollment_print::PAINTER,
        Box::new(move |region, input| print.paint(region, input, colors)),
    );
    runtime.register_canvas(
        enrollment_ring::PAINTER,
        Box::new(move |region, input| enrollment_ring::paint(region, input, colors)),
    );
    window_theme::apply(&control, appearance);
    control.apply(&mut runtime, &mut css);

    let mut dashboard = Dashboard::new(Localizer::system(carosign_l10n::Preferences::new(
        "ru", None,
    )))
    .unwrap();
    dashboard.enable_fingerprint_demo();
    let dashboard = Rc::new(dashboard);
    let mut navigation = Navigation::default();
    navigation.resize(size.width);
    let navigation = Rc::new(Cell::new(navigation));
    for page in [Page::Platform, Page::Fingerprints, Page::Policies] {
        let navigation = Rc::clone(&navigation);
        runtime.on_command(page.command(), move || {
            let mut state = navigation.get();
            state.enter(page);
            navigation.set(state);
        });
    }
    let back = Rc::clone(&navigation);
    runtime.on_command(InternedSymbol::named("navigate_back"), move || {
        let mut state = back.get();
        state.back();
        back.set(state);
    });
    let selection_control = control.clone();
    let selection_dashboard = Rc::clone(&dashboard);
    runtime.on_command(
        InternedSymbol::named("fingerprint_select"),
        move |event: &mut UiEvent| {
            let target = event.target.unwrap();
            let dashboard = Rc::clone(&selection_dashboard);
            selection_control.with_runtime(move |runtime| {
                assert!(dashboard.select_registered_fingerprint(runtime, target));
            });
        },
    );
    for command in [
        "fingerprint_rename",
        "fingerprint_add",
        "fingerprint_delete",
        "fingerprint_delete_cancel",
    ] {
        let dashboard = Rc::clone(&dashboard);
        runtime.on_command(InternedSymbol::named(command), move || {
            assert!(dashboard.fingerprint_command(command));
        });
    }
    let enrollment = Rc::clone(&dashboard);
    let inventory_dashboard = Rc::clone(&dashboard);
    let inventory_control = control.clone();
    runtime.on_command(
        InternedSymbol::named("inventory_toggle_device"),
        move |event: &mut UiEvent| {
            let target = event.target.unwrap();
            let dashboard = Rc::clone(&inventory_dashboard);
            inventory_control.with_runtime(move |runtime| {
                assert!(dashboard.toggle_inventory_device(runtime, target));
            });
        },
    );
    let restart = Rc::clone(&dashboard);
    let action_dashboard = Rc::clone(&dashboard);
    let action_control = control.clone();
    runtime.on_command(
        InternedSymbol::named("inventory_device_action"),
        move |event: &mut UiEvent| {
            event.stop_propagation();
            let target = event.target.unwrap();
            let dashboard = Rc::clone(&action_dashboard);
            action_control.with_runtime(move |runtime| {
                let (id, disable) = dashboard.begin_device_action(runtime, target).unwrap();
                assert_eq!(id, "abababababababababababababababab");
                assert!(disable);
            });
        },
    );
    runtime.on_command(InternedSymbol::named("policy_restart_service"), move || {
        assert!(matches!(
            restart.policies().command("policy_restart_service"),
            Some(PolicyJob::RestartService)
        ));
    });

    let connection = Rc::new(Cell::new(None));
    let rendered_connection = Rc::clone(&connection);
    let degraded = Rc::new(Cell::new(false));
    let rendered_degraded = Rc::clone(&degraded);
    let online = connected_snapshot();
    let degraded_snapshot = Snapshot {
        health: Err(QueryError::Unavailable),
        ..online.clone()
    };
    let offline = Snapshot {
        health: Err(QueryError::Unavailable),
        inventory: Err(QueryError::Unavailable),
        enrollments: Err(QueryError::Unavailable),
    };
    let error_preview = Rc::new(Cell::new(false));
    let render_error = Rc::clone(&error_preview);
    let error_dashboard = Dashboard::new(Localizer::system(carosign_l10n::Preferences::new(
        "ru", None,
    )))
    .unwrap();
    let unavailable_enrollments = Snapshot {
        enrollments: Err(QueryError::Unavailable),
        ..online.clone()
    };
    let compact = navigation.get().compact();
    let mut screens = ScreenTransition::default();
    let stage_control = control.clone();
    runtime.set_root_view(move || {
        let layout = navigation.get();
        let page = layout.page();
        if render_error.get() {
            screens.update(
                Page::Fingerprints as u64,
                error_dashboard.view(
                    Page::Fingerprints,
                    Some(&unavailable_enrollments),
                    layout.compact(),
                    false,
                ),
                false,
                false,
            );
            return screens.view(size.width, layout.compact(), &stage_control);
        }
        screens.update(
            page as u64,
            dashboard.view(
                page,
                rendered_connection.get().map(|connected| {
                    match (connected, rendered_degraded.get()) {
                        (true, true) => &degraded_snapshot,
                        (true, false) => &online,
                        (false, _) => &offline,
                    }
                }),
                layout.compact(),
                false,
            ),
            false,
            false,
        );
        screens.view(size.width, layout.compact(), &stage_control)
    });
    let mut capture = |runtime: &UiRuntime, state: &str| {
        let frame = RenderFrame::new(runtime.snapshot(), size);
        let (width, height, pixels) = renderer
            .render_to_image(&FrameSubmission::new(frame), width, height)
            .unwrap();
        let image = tiny_skia::Pixmap::from_vec(
            pixels,
            tiny_skia::IntSize::from_wh(width, height).unwrap(),
        )
        .unwrap();
        image
            .save_png(output.join(format!("{prefix}-{state}.png")))
            .unwrap();
        image
    };
    render_frame(&mut runtime);
    capture(&runtime, "startup");
    if !compact {
        let node = SelectorResolver::new()
            .resolve(&runtime, &SelectorTarget::test_id("platform-navigation"))
            .unwrap();
        let semantics = runtime.semantic_snapshot();
        let name = semantics.get(node).and_then(|node| node.name()).unwrap();
        assert_eq!(name, "Платформа");
    }
    click(&mut runtime, "platform-navigation");
    render_frame(&mut runtime);
    capture(&runtime, "platform-loading");
    connection.set(Some(false));
    render_frame(&mut runtime);
    capture(&runtime, "platform-offline");
    if compact {
        let button = SelectorResolver::new()
            .resolve(
                &runtime,
                &SelectorTarget::test_id("platform-service-restart"),
            )
            .unwrap();
        let semantics = runtime.semantic_snapshot();
        assert_eq!(
            semantics.get(button).and_then(|node| node.name()),
            Some("Перезапустить службу")
        );
        assert!(runtime.focus_node(Some(button)));
        let enter = Some(LogicalKey::named(NamedKey::Enter));
        runtime.handle_key(KeyEvent::press(PhysicalKey::Enter, enter));
        runtime.handle_key(KeyEvent::release(PhysicalKey::Enter, enter));
        render_frame(&mut runtime);
        assert!(
            runtime
                .semantic_snapshot()
                .get(button)
                .unwrap()
                .state()
                .disabled
        );
        enrollment
            .policies()
            .finish(Err("ui-policy-restart-failed"));
        render_frame(&mut runtime);
        assert!(
            !runtime
                .semantic_snapshot()
                .get(button)
                .unwrap()
                .state()
                .disabled
        );
        capture(&runtime, "platform-restart-failed");
    }
    connection.set(Some(true));
    render_frame(&mut runtime);
    let platform = capture(&runtime, "platform");
    let pane_left = if compact { 0 } else { 248 };
    let surface = platform.pixel(pane_left + 10, 60);
    for x in [pane_left + 10, width - 12] {
        assert_eq!(platform.pixel(x, 4), surface, "{prefix}: header surface");
        assert_eq!(
            platform.pixel(x, 60),
            surface,
            "{prefix}: inventory surface"
        );
    }
    if !compact {
        let nodes = runtime.inspect_snapshot().nodes;
        let [inventory, service] = ["inventory-content", "platform-summary"].map(|class| {
            let node = nodes
                .iter()
                .find(|node| node.classes.iter().any(|name| name == class))
                .unwrap();
            let bounds = runtime.layout_boxes().get(&node.node).unwrap();
            assert!(
                bounds.origin().x + bounds.size().width <= size.width + 0.5,
                "{prefix}: {class} exceeds the content pane"
            );
            *bounds
        });
        assert!(inventory.origin().y + inventory.size().height < service.origin().y);
        assert!((inventory.origin().x - service.origin().x).abs() < 0.5);
        assert!((inventory.size().width - service.size().width).abs() < 0.5);
        assert!(inventory.size().width <= 680.5);
        let column_center = inventory.origin().x + inventory.size().width / 2.0;
        assert!(
            (column_center - (size.width + 248.0) / 2.0).abs() < 0.5,
            "{prefix}: platform column is off-center at {column_center}"
        );
    }
    if compact {
        let selector = "inventory-device-abababababababababababababababab";
        let expanded = |runtime: &UiRuntime| {
            runtime
                .inspect_snapshot()
                .nodes
                .iter()
                .filter(|node| {
                    node.classes
                        .iter()
                        .any(|class| class == "device-disclosure")
                })
                .count()
        };
        assert_eq!(expanded(&runtime), 0);
        let card = SelectorResolver::new()
            .resolve(
                &runtime,
                &SelectorTarget::test_id("inventory-card-abababababababababababababababab"),
            )
            .unwrap();
        click_card_body(&mut runtime, card);
        control.apply(&mut runtime, &mut css);
        render_frame(&mut runtime);
        assert_eq!(expanded(&runtime), 1);
        capture(&runtime, "platform-device-expanded");
        let action = "inventory-disable-abababababababababababababababab";
        click(&mut runtime, action);
        control.apply(&mut runtime, &mut css);
        render_frame(&mut runtime);
        assert_eq!(expanded(&runtime), 1);
        let action = SelectorResolver::new()
            .resolve(&runtime, &SelectorTarget::test_id(action))
            .unwrap();
        assert!(
            runtime
                .semantic_snapshot()
                .get(action)
                .unwrap()
                .state()
                .disabled
        );
        enrollment.finish_device_action("ui-device-action-denied");
        render_frame(&mut runtime);
        assert!(
            !runtime
                .semantic_snapshot()
                .get(action)
                .unwrap()
                .state()
                .disabled
        );
        let identifier = runtime
            .inspect_snapshot()
            .nodes
            .into_iter()
            .find(|node| node.classes.iter().any(|class| class == "identifier"))
            .unwrap()
            .node;
        let bounds = runtime.layout_boxes().get(&identifier).unwrap();
        let point = Point::new(
            bounds.origin().x + 8.0,
            bounds.origin().y + bounds.size().height / 2.0,
        );
        click_at(&mut runtime, point);
        control.apply(&mut runtime, &mut css);
        render_frame(&mut runtime);
        assert_eq!(expanded(&runtime), 0);
        click(&mut runtime, selector);
        control.apply(&mut runtime, &mut css);
        render_frame(&mut runtime);
        assert_eq!(expanded(&runtime), 1);
        let button = SelectorResolver::new()
            .resolve(&runtime, &SelectorTarget::test_id(selector))
            .unwrap();
        assert!(runtime.focus_node(Some(button)));
        let enter = Some(LogicalKey::named(NamedKey::Enter));
        runtime.handle_key(KeyEvent::press(PhysicalKey::Enter, enter));
        runtime.handle_key(KeyEvent::release(PhysicalKey::Enter, enter));
        control.apply(&mut runtime, &mut css);
        render_frame(&mut runtime);
        assert_eq!(expanded(&runtime), 0);
        let nodes = runtime.inspect_snapshot().nodes;
        for (class, inset) in [("platform-summary", 0.0), ("inventory-list", 24.0)] {
            let node = nodes
                .iter()
                .find(|node| node.classes.iter().any(|name| name == class))
                .unwrap();
            let bounds = runtime.layout_boxes().get(&node.node).unwrap();
            assert!(
                (bounds.origin().x - inset).abs() < 0.5,
                "{prefix}: {class} has the wrong left gutter"
            );
            assert!(
                (bounds.size().width - (size.width - 2.0 * inset)).abs() < 0.5,
                "{prefix}: {class} has the wrong width"
            );
        }
    }
    if prefix == "compact-contrast" {
        let last_metric = runtime
            .inspect_snapshot()
            .nodes
            .into_iter()
            .rfind(|node| {
                node.classes
                    .iter()
                    .any(|class| class == "platform-metric-label")
            })
            .unwrap()
            .node;
        runtime.scroll_into_view(last_metric);
        render_frame(&mut runtime);
        let bounds = runtime.layout_boxes().get(&last_metric).unwrap();
        assert!(bounds.origin().y + bounds.size().height <= size.height);
        capture(&runtime, "platform-scrolled");
    }
    if width >= 1100 {
        let list_position = |runtime: &UiRuntime| {
            let node = runtime
                .inspect_snapshot()
                .nodes
                .into_iter()
                .find(|node| node.classes.iter().any(|class| class == "inventory-list"))
                .unwrap()
                .node;
            runtime.layout_boxes().get(&node).unwrap().origin()
        };
        let position = list_position(&runtime);
        degraded.set(true);
        render_frame(&mut runtime);
        assert_eq!(list_position(&runtime), position);
        capture(&runtime, "platform-degraded");
        degraded.set(false);
    }
    if compact {
        click(&mut runtime, "navigation-back");
        render_frame(&mut runtime);
    }
    let policies = SelectorResolver::new()
        .resolve(&runtime, &SelectorTarget::test_id("policies-navigation"))
        .unwrap();
    runtime.scroll_into_view(policies);
    render_frame(&mut runtime);
    click(&mut runtime, "policies-navigation");
    render_frame(&mut runtime);
    capture(&runtime, "policies");
    assert!(matches!(
        enrollment.policies().command("policy_unlock"),
        Some(PolicyJob::Read)
    ));
    enrollment.policies().finish(Ok(PolicyResult::Loaded {
        document: String::new(),
        accounts: None,
    }));
    render_frame(&mut runtime);
    let rules = capture(&runtime, "policies-rules");
    assert_section_band(&runtime, &rules, pane_left, width);
    enrollment.policies().command("policy_add");
    render_frame(&mut runtime);
    let editor = capture(&runtime, "policies-editor");
    assert_section_band(&runtime, &editor, pane_left, width);
    enrollment.policies().command("policy_close_editor");

    if compact {
        click(&mut runtime, "navigation-back");
        render_frame(&mut runtime);
    }
    for (selector, state) in [
        ("fingerprints-navigation", "collapsed"),
        ("fingerprint-row-1", "first"),
        ("fingerprint-row-3", "last"),
        ("fingerprint-delete", "confirm-delete"),
        ("fingerprint-delete-cancel", "cancel-delete"),
        ("fingerprint-rename", "editing-last"),
    ] {
        if state == "cancel-delete" {
            let cancel = SelectorResolver::new()
                .resolve(&runtime, &SelectorTarget::test_id(selector))
                .unwrap();
            runtime.scroll_into_view(cancel);
            render_frame(&mut runtime);
        }
        click(&mut runtime, selector);
        control.apply(&mut runtime, &mut css);
        render_frame(&mut runtime);
        if state == "editing-last" {
            account_menu_scroll::focus(&mut runtime, "fingerprint-inline-name");
            render_frame(&mut runtime);
        }
        let image = capture(&runtime, state);
        assert_section_band(&runtime, &image, pane_left, width);
        assert_list_corners(&runtime, &image, prefix, state);
        if state == "last" {
            let row = control_center(&runtime, "fingerprint-row-3");
            move_pointer(&mut runtime, row);
            render_frame(&mut runtime);
            let hover = capture(&runtime, "last-row-hover");
            assert!(
                image.data() == hover.data(),
                "{prefix}: expanded row changed on hover"
            );

            let action = control_center(&runtime, "fingerprint-rename");
            move_pointer(&mut runtime, action);
            render_frame(&mut runtime);
            let hover = capture(&runtime, "last-action-hover");
            assert!(
                image.data() != hover.data(),
                "{prefix}: action has no hover feedback"
            );
        }
    }
    click(&mut runtime, "fingerprint-add");
    for _ in 0..10 {
        assert!(enrollment.fingerprint_command("fingerprint_next"));
    }
    render_frame(&mut runtime);
    capture(&runtime, "enrollment");
    error_preview.set(true);
    render_frame(&mut runtime);
    let unavailable = capture(&runtime, "fingerprints-unavailable");
    assert_eq!(
        unavailable.pixel(pane_left + 12, 4),
        unavailable.pixel(pane_left + 12, 68)
    );
    let header = runtime
        .inspect_snapshot()
        .nodes
        .into_iter()
        .find(|node| node.classes.iter().any(|class| class == "header"))
        .unwrap();
    assert_eq!(
        runtime
            .computed_style(header.node)
            .unwrap()
            .get(obscur_kit::style::property::PropertyId::BackgroundColor)
            .and_then(obscur_kit::style::computed::ComputedValue::as_color),
        Some(obscur_kit::style::property::StyleColor::TRANSPARENT),
    );
    let header_bottom = runtime
        .layout_boxes()
        .get(&header.node)
        .unwrap()
        .bounds
        .bottom() as u32;
    assert_eq!(
        unavailable.pixel(pane_left + 12, header_bottom - 1),
        unavailable.pixel(pane_left + 12, header_bottom),
        "{prefix}: the header edge should blend into the content",
    );
}

fn connected_snapshot() -> Snapshot {
    let id = vec![0xab; 16];
    Snapshot {
        health: Ok(Health {
            service_ready: true,
            available_authenticators: 1,
            unavailable_authenticators: 1,
            ..Health::default()
        }),
        inventory: Ok(Authenticators {
            entries: vec![
                AuthenticatorDescriptor {
                    authenticator_id: id.clone(),
                    available: true,
                    ..AuthenticatorDescriptor::default()
                },
                AuthenticatorDescriptor {
                    authenticator_id: vec![0xcd; 16],
                    available: false,
                    unavailability_reason: UnavailabilityReason::InventoryOnly as i32,
                    ..AuthenticatorDescriptor::default()
                },
            ],
            providers: Some(ProviderInventory {
                publications: vec![
                    ProviderPublication {
                        name: "carosign-mock".into(),
                        reachable: true,
                        authenticator_ids: vec![id, vec![0xcd; 16]],
                        ..ProviderPublication::default()
                    },
                    ProviderPublication {
                        name: "carosign-remote".into(),
                        reachable: false,
                        ..ProviderPublication::default()
                    },
                ],
            }),
        }),
        enrollments: Ok(Vec::new()),
    }
}

fn render_frame(runtime: &mut UiRuntime) {
    runtime.request_frame(FrameReason::UiCommit);
    runtime.run_due_frame_reactive().unwrap();
    while runtime.poll_resource_tasks() > 0 {
        runtime.run_due_frame_reactive().unwrap();
    }
}

fn assert_section_band(runtime: &UiRuntime, image: &tiny_skia::Pixmap, pane_left: u32, width: u32) {
    let snapshot = runtime.inspect_snapshot();
    let bounds = |class: &str| {
        let node = snapshot
            .nodes
            .iter()
            .find(|node| node.classes.iter().any(|name| name == class))
            .unwrap();
        *runtime.layout_boxes().get(&node.node).unwrap()
    };
    let band = bounds("section-band");
    let column = bounds("section-column");
    let center = (pane_left + width) as f32 / 2.0;

    assert!((band.origin().x - pane_left as f32).abs() < 0.5);
    assert!((band.size().width - (width - pane_left) as f32).abs() < 0.5);
    assert!(column.size().width <= 680.5);
    assert!((column.origin().x + column.size().width / 2.0 - center).abs() < 0.5);
    for x in [pane_left + 12, width - 14] {
        assert_eq!(
            image.pixel(x, 4),
            image.pixel(x, band.origin().y as u32 + 4)
        );
    }
}

fn assert_list_corners(runtime: &UiRuntime, image: &tiny_skia::Pixmap, prefix: &str, state: &str) {
    let list = runtime
        .inspect_snapshot()
        .nodes
        .into_iter()
        .find(|node| node.classes.iter().any(|class| class == "fingerprint-rows"))
        .unwrap();
    let bounds = runtime.layout_boxes().get(&list.node).unwrap();
    let left = bounds.origin().x.ceil() as u32;
    let right = (bounds.origin().x + bounds.size().width).floor() as u32 - 1;
    let top = bounds.origin().y.ceil() as u32;
    let bottom = (bounds.origin().y + bounds.size().height).floor() as u32 - 1;
    for (x, outside) in [(left + 1, left - 2), (right - 1, right + 2)] {
        for y in [top + 1, bottom - 1] {
            assert_eq!(
                image.pixel(x, y),
                image.pixel(outside, y),
                "{prefix} {state}: fill protrudes into the rounded corner at ({x}, {y})",
            );
        }
    }
}

fn control_center(runtime: &UiRuntime, selector: &str) -> Point {
    let node = SelectorResolver::new()
        .resolve(runtime, &SelectorTarget::test_id(selector))
        .unwrap_or_else(|| panic!("missing control {selector}"));
    let bounds = runtime
        .layout_boxes()
        .get(&node)
        .unwrap_or_else(|| panic!("missing bounds for {selector}"));
    Point::new(
        bounds.origin().x + bounds.size().width / 2.0,
        bounds.origin().y + bounds.size().height / 2.0,
    )
}

fn click(runtime: &mut UiRuntime, selector: &str) {
    let center = control_center(runtime, selector);
    click_at(runtime, center);
}

fn click_card_body(runtime: &mut UiRuntime, card: obscur_kit::identity::NodeId) {
    let bounds = runtime.layout_boxes().get(&card).unwrap();
    let point = Point::new(
        bounds.origin().x + bounds.size().width / 2.0,
        bounds.origin().y + bounds.size().height - 6.0,
    );
    click_at(runtime, point);
}

fn click_at(runtime: &mut UiRuntime, center: Point) {
    for kind in [PointerEventKind::Down, PointerEventKind::Up] {
        runtime.handle_pointer(
            PointerEvent::new(PointerId::MOUSE, PointerDevice::Mouse, kind, center).as_trusted(),
        );
    }
    move_pointer(runtime, Point::new(0.0, 0.0));
}

fn move_pointer(runtime: &mut UiRuntime, point: Point) {
    runtime.handle_pointer(
        PointerEvent::new(
            PointerId::MOUSE,
            PointerDevice::Mouse,
            PointerEventKind::Move,
            point,
        )
        .as_trusted(),
    );
}
