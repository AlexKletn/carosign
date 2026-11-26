use std::cell::{Cell, RefCell};
use std::error::Error;
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::Arc;

use carosign_l10n::Localizer;
use obscur_kit::identity::InternedSymbol;
use obscur_kit::interaction::{
    command::NamedKey,
    event::UiEvent,
    pointer::{PointerButton, PointerEventKind},
};
use obscur_kit::{App, RenderBackend};

use crate::{
    ipc,
    ui::widgets::{Policies, PolicyJob, PolicyResult, WidgetImages},
};

use super::{
    Dashboard, account_menu_scroll, screen_transition::ScreenTransition, window_controls,
    window_theme,
};
use crate::ui::{navigation::Navigation, page::Page};
use obscur_kit::interaction::gesture::{BackGesture, BackGesturePhase};

pub(crate) fn run() -> Result<(), Box<dyn Error>> {
    let localizer = Localizer::from_environment();
    let title = localizer.text("ui-title");
    let chrome_localizer = Localizer::from_environment();
    let mut dashboard = Dashboard::new(localizer)?;
    if std::env::var("CAROSIGN_GUI_DEMO_FINGERPRINTS").as_deref() == Ok("1") {
        dashboard.enable_fingerprint_demo();
    }
    let dashboard = Rc::new(dashboard);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let navigation = Rc::new(Cell::new(Navigation::default()));
    let policy_edited = Rc::new(Cell::new(false));
    let (policy_sender, policy_receiver) = std::sync::mpsc::channel();
    let policy_waker: Rc<RefCell<Option<Arc<dyn Fn() -> bool + Send + Sync>>>> = Rc::default();
    let device_waker = Rc::clone(&policy_waker);
    let fingerprint_waker = Rc::clone(&policy_waker);
    let policy_window = Rc::clone(&policy_waker);
    let (probe_sender, probe_receiver) = std::sync::mpsc::sync_channel(1);
    let mut rendered_page = navigation.get().page();
    let mut rendered_compact = None;
    let mut rendered_motion = false;
    let mut snapshot = None;
    let mut account_focus = None;
    let mut registered_account_choices = 0;
    let mut rendered_editor = false;
    let mut enrollment_viewport = None;
    let mut screens = ScreenTransition::default();
    let theme = window_theme::WindowTheme::default();
    let print_theme = theme.clone();
    let ring_theme = theme.clone();
    let mut fingerprint = crate::ui::widgets::enrollment_print::EnrollmentPrint::default();
    let app = App::new(title)
        .with_renderer(RenderBackend::Lyon)
        .system_theme(theme.clone())
        .stylesheet(window_theme::STYLESHEET)
        .app_id("org.carosign.Ui")
        .with_decorations(false)
        .with_resize(true)
        .with_size(780.0, 580.0)
        .with_min_size(480.0, 360.0)
        .with_max_size(1200.0, 900.0)
        .painter_id(
            crate::ui::widgets::enrollment_print::PAINTER,
            move |region, input| fingerprint.paint(region, input, print_theme.enrollment_colors()),
        )
        .painter_id(
            crate::ui::widgets::enrollment_ring::PAINTER,
            move |region, input| {
                crate::ui::widgets::enrollment_ring::paint(
                    region,
                    input,
                    ring_theme.enrollment_colors(),
                )
            },
        );
    let control = app.control();
    let drag_dashboard = Rc::clone(&dashboard);
    let drag_policy_edited = Rc::clone(&policy_edited);
    let drag_control = control.clone();
    let app = window_controls::bind(app, &control, move || {
        if !drag_dashboard.policies().dismiss_account_menu() {
            return false;
        }
        drag_policy_edited.set(true);
        drag_control.request_frame();
        true
    });
    WidgetImages::install(&control);
    let draft = Rc::clone(&dashboard);
    let draft_edited = Rc::clone(&policy_edited);
    control.with_runtime(move |runtime| {
        for field in Policies::FIELDS {
            let draft = Rc::clone(&draft);
            let edited = Rc::clone(&draft_edited);
            runtime.on_text_input(InternedSymbol::named(field), move |text| {
                draft.policies().edit(field, text);
                edited.set(true);
            });
        }
    });
    let launch_policy_job = Rc::new(move |job: PolicyJob| {
        let wake = policy_waker.borrow().clone();
        let sender = policy_sender.clone();
        std::thread::spawn(move || {
            if sender.send(job.run()).is_ok()
                && let Some(wake) = wake
            {
                wake();
            }
        });
    });
    let app = Policies::commands().fold(app, |app, command| {
        let dashboard = Rc::clone(&dashboard);
        let edited = Rc::clone(&policy_edited);
        let launch = Rc::clone(&launch_policy_job);
        let control = control.clone();
        app.on_command(InternedSymbol::named(command), move || {
            edited.set(true);
            if let Some(job) = dashboard.policies().command(command) {
                launch(job);
            }
            if matches!(command, "policy_caller" | "policy_subject")
                && let Some(trigger) = dashboard.policies().account_menu_trigger()
            {
                control.with_runtime(move |runtime| account_menu_scroll::reveal(runtime, trigger));
            }
        })
    });
    let app = ["policy_account_escape", "select_search_key"]
        .into_iter()
        .fold(app, |app, command| {
            let dashboard = Rc::clone(&dashboard);
            let edited = Rc::clone(&policy_edited);
            let control = control.clone();
            app.on_command(
                InternedSymbol::named(command),
                move |event: &mut UiEvent| {
                    let Some(trigger) = dashboard.policies().account_menu_trigger() else {
                        return;
                    };
                    let Some(key) = event.key().and_then(|key| key.logical?.named_key()) else {
                        return;
                    };
                    match key {
                        NamedKey::Escape => {
                            dashboard.policies().dismiss_account_menu();
                            control.with_runtime(move |runtime| {
                                account_menu_scroll::focus(runtime, trigger)
                            });
                        }
                        NamedKey::Enter if command == "select_search_key" => {
                            dashboard.policies().choose_active_account();
                            if dashboard.policies().account_menu_trigger().is_none() {
                                control.with_runtime(move |runtime| {
                                    account_menu_scroll::focus(runtime, trigger)
                                });
                            }
                        }
                        NamedKey::ArrowUp | NamedKey::ArrowDown
                            if command == "select_search_key" =>
                        {
                            let index = dashboard
                                .policies()
                                .move_account_cursor(key == NamedKey::ArrowDown);
                            control.with_runtime(move |runtime| {
                                let id = format!("select-result-{index}");
                                let row = runtime
                                    .inspect_snapshot()
                                    .nodes
                                    .into_iter()
                                    .find(|node| node.id.as_deref() == Some(id.as_str()));
                                if let Some(row) = row {
                                    runtime.scroll_into_view(row.node);
                                }
                            });
                        }
                        _ => return,
                    }
                    edited.set(true);
                    event.prevent_default();
                },
            )
        });
    let app = [Page::Platform, Page::Fingerprints, Page::Policies]
        .into_iter()
        .fold(app, |app, section| {
            let navigation = Rc::clone(&navigation);
            let dashboard = Rc::clone(&dashboard);
            let edited = Rc::clone(&policy_edited);
            app.on_command(section.command(), move || {
                let mut state = navigation.get();
                state.enter(section);
                navigation.set(state);
                if section == Page::Policies && dashboard.policies().is_editing() {
                    dashboard.policies().command("policy_close_editor");
                    edited.set(true);
                }
            })
        });
    let fingerprints_changed = Rc::new(Cell::new(false));
    let back_fingerprints_changed = Rc::clone(&fingerprints_changed);
    let back_navigation = Rc::clone(&navigation);
    let back_dashboard = Rc::clone(&dashboard);
    let back_edited = Rc::clone(&policy_edited);
    let go_back = Rc::new(move || {
        let mut state = back_navigation.get();
        if !back_dashboard.can_go_back(state.page(), state.compact()) {
            return false;
        }
        if state.page() == Page::Fingerprints && back_dashboard.cancel_fingerprint_enrollment() {
            back_fingerprints_changed.set(true);
            return true;
        }
        if state.page() == Page::Policies && back_dashboard.policies().is_editing() {
            back_dashboard.policies().command("policy_close_editor");
            back_edited.set(true);
        } else {
            state.back();
            back_navigation.set(state);
        }
        true
    });
    let fingerprint_focus = Rc::new(RefCell::new(None));
    let (fingerprint_sender, fingerprint_receiver) = std::sync::mpsc::channel();
    let fingerprint_command = {
        let dashboard = Rc::clone(&dashboard);
        let changed = Rc::clone(&fingerprints_changed);
        let focus = Rc::clone(&fingerprint_focus);
        Rc::new(move |command: &str| {
            if !dashboard.fingerprint_command(command) {
                let (handled, job) = dashboard.run_live_fingerprint_command(command);
                if !handled {
                    return;
                }
                if let Some(job) = job {
                    let sender = fingerprint_sender.clone();
                    let wake = fingerprint_waker.borrow().clone();
                    std::thread::spawn(move || {
                        job.run(|update| {
                            if sender.send(update).is_ok() && let Some(wake) = &wake {
                                wake();
                            }
                        });
                    });
                }
            }
            changed.set(true);
            match command {
                "fingerprint_rename" => {
                    *focus.borrow_mut() = Some("fingerprint-inline-name".to_owned());
                }
                "fingerprint_rename_save" | "fingerprint_rename_cancel" => {
                    *focus.borrow_mut() = dashboard.selected_fingerprint_row_id();
                }
                _ => {}
            }
        })
    };
    let app = [
        "fingerprint_add",
        "fingerprint_next",
        "fingerprint_discard",
        "fingerprint_rename",
        "fingerprint_rename_save",
        "fingerprint_rename_cancel",
        "fingerprint_delete",
        "fingerprint_delete_cancel",
        "fingerprint_delete_confirm",
    ]
    .into_iter()
    .fold(app, |app, command| {
        let dispatch = Rc::clone(&fingerprint_command);
        app.on_command(InternedSymbol::named(command), move || dispatch(command))
    });
    let app = app.on_command(
        InternedSymbol::named("fingerprint_rename_key"),
        move |event: &mut UiEvent| {
            let Some(key) = event.key().and_then(|key| key.logical?.named_key()) else {
                return;
            };
            let command = match key {
                NamedKey::Enter => "fingerprint_rename_save",
                NamedKey::Escape => "fingerprint_rename_cancel",
                _ => return,
            };
            fingerprint_command(command);
            event.prevent_default();
        },
    );
    let name_dashboard = Rc::clone(&dashboard);
    let name_changed = Rc::clone(&fingerprints_changed);
    let rename_dashboard = Rc::clone(&dashboard);
    let rename_changed = Rc::clone(&fingerprints_changed);
    let row_dashboard = Rc::clone(&dashboard);
    let row_changed = Rc::clone(&fingerprints_changed);
    let row_control = control.clone();
    let inventory_changed = Rc::new(Cell::new(false));
    let inventory_event_changed = Rc::clone(&inventory_changed);
    let inventory_dashboard = Rc::clone(&dashboard);
    let inventory_control = control.clone();
    let (device_sender, device_receiver) = std::sync::mpsc::channel();
    let device_dashboard = Rc::clone(&dashboard);
    let device_control = control.clone();
    let device_changed = Rc::clone(&inventory_changed);
    let app = app.on_command(
        InternedSymbol::named("inventory_device_action"),
        move |event: &mut UiEvent| {
            event.stop_propagation();
            let Some(target) = event.target else { return };
            let dashboard = Rc::clone(&device_dashboard);
            let sender = device_sender.clone();
            let wake = device_waker.borrow().clone();
            let changed = Rc::clone(&device_changed);
            device_control.with_runtime(move |runtime| {
                let Some((id, disable)) = dashboard.begin_device_action(runtime, target) else {
                    return;
                };
                changed.set(true);
                runtime.request_frame(obscur_kit::runtime::frame::FrameReason::UiCommit);
                std::thread::spawn(move || {
                    let result = ipc::device_admin::set_disabled(&id, disable);
                    if sender.send(result).is_ok()
                        && let Some(wake) = wake
                    {
                        wake();
                    }
                });
            });
        },
    );
    let app = app.on_command(
        InternedSymbol::named("inventory_toggle_device"),
        move |event: &mut UiEvent| {
            let Some(target) = event.target else { return };
            let dashboard = Rc::clone(&inventory_dashboard);
            let changed = Rc::clone(&inventory_event_changed);
            inventory_control.with_runtime(move |runtime| {
                if dashboard.toggle_inventory_device(runtime, target) {
                    changed.set(true);
                    runtime.request_frame(obscur_kit::runtime::frame::FrameReason::UiCommit);
                }
            });
        },
    );
    let app = app.on_command(
        InternedSymbol::named("fingerprint_select"),
        move |event: &mut UiEvent| {
            let Some(target) = event.target else { return };
            let dashboard = Rc::clone(&row_dashboard);
            let changed = Rc::clone(&row_changed);
            row_control.with_runtime(move |runtime| {
                if dashboard.select_registered_fingerprint(runtime, target) {
                    changed.set(true);
                    runtime.request_frame(obscur_kit::runtime::frame::FrameReason::UiCommit);
                }
            });
        },
    );
    control.with_runtime(move |runtime| {
        runtime.on_text_input(InternedSymbol::named("fingerprint_name"), move |name| {
            if name_dashboard.edit_fingerprint_name(name) {
                name_changed.set(true);
            }
        });
        runtime.on_text_input(
            InternedSymbol::named("fingerprint_rename_input"),
            move |name| {
                if rename_dashboard.edit_registered_fingerprint_name(name) {
                    rename_changed.set(true);
                }
            },
        );
    });
    let scale = Rc::new(Cell::new(1.0));
    let gesture_scale = Rc::clone(&scale);
    let gesture_navigation = Rc::clone(&navigation);
    let gesture_dashboard = Rc::clone(&dashboard);
    let back_swipe = Rc::new(RefCell::new(
        BackGesture::default().with_touchpad_finger_count(NonZeroUsize::new(2).unwrap()),
    ));
    let gesture_swipe = Rc::clone(&back_swipe);
    let gesture_control = control.clone();
    let gesture_edited = Rc::clone(&policy_edited);
    let button_back = Rc::clone(&go_back);
    app.on_command(InternedSymbol::named("navigate_back"), move || {
        button_back();
    })
    .on_command(
        InternedSymbol::named("navigation_swipe"),
        move |event: &mut UiEvent| {
            use obscur_kit::interaction::event::EventOrigin;

            let pointer = event.pointer();
            if pointer.is_some_and(|pointer| {
                pointer.kind() == PointerEventKind::Up
                    && pointer.button() == Some(PointerButton::Back)
            }) {
                if go_back() {
                    event.prevent_default();
                }
                return;
            }
            if event.origin != EventOrigin::Gesture
                && pointer.is_some_and(|pointer| pointer.kind() == PointerEventKind::Down)
                && gesture_dashboard.policies().dismiss_account_menu()
            {
                gesture_edited.set(true);
                gesture_swipe.borrow_mut().cancel();
                gesture_control.request_frame();
                return;
            }
            if pointer.is_some() && event.origin != EventOrigin::Gesture {
                return;
            }
            let state = gesture_navigation.get();
            let mut swipe = gesture_swipe.borrow_mut();
            if !gesture_dashboard.can_go_back(state.page(), state.compact())
                || gesture_dashboard
                    .policies()
                    .account_menu_trigger()
                    .is_some()
            {
                if swipe.cancel().is_some() {
                    gesture_control.request_frame();
                }
                return;
            }
            if let Some(update) = swipe.update(event, gesture_scale.get()) {
                if update.phase == BackGesturePhase::Completed {
                    go_back();
                }
                gesture_control.request_frame();
                event.prevent_default();
            }
        },
    )
    .on_window_waker(move |wake| {
        let wake: Arc<dyn Fn() -> bool + Send + Sync> = Arc::from(wake);
        *policy_window.borrow_mut() = Some(Arc::clone(&wake));
        if std::env::var_os("CAROSIGN_FRAME_PROBE").is_some() {
            let probe_wake = Arc::clone(&wake);
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(1));
                for page in [Page::Platform, Page::Fingerprints]
                    .into_iter()
                    .cycle()
                    .take(8)
                {
                    if probe_sender.send(page).is_err() || !probe_wake() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(750));
                }
            });
        }
        ipc::start(sender, move || wake());
    })
    .view_with(move |frame| {
        scale.set(frame.scale.max(0.05));
        let mut layout = navigation.get();
        layout.resize(frame.viewport.width);
        if let Ok(page) = probe_receiver.try_recv() {
            layout.enter(page);
            println!("[carosign-frame-probe] page={}", page as u64);
        }
        navigation.set(layout);
        let current_page = layout.page();
        let compact = layout.compact();
        let motion = theme.motion_enabled();
        let update = receiver.try_recv().ok();
        if let Ok(result) = policy_receiver.try_recv() {
            if let Ok(PolicyResult::Loaded {
                accounts: Some(accounts),
                ..
            }) = &result
                && accounts.len() > registered_account_choices
            {
                let indices = registered_account_choices..accounts.len();
                registered_account_choices = accounts.len();
                let dashboard = Rc::clone(&dashboard);
                let edited = Rc::clone(&policy_edited);
                control.with_runtime(move |runtime| {
                    for index in indices {
                        let dashboard = Rc::clone(&dashboard);
                        let edited = Rc::clone(&edited);
                        runtime.on_command(
                            InternedSymbol::named(&format!("policy_account_{index}")),
                            move || {
                                dashboard.policies().choose_account(index);
                                edited.set(true);
                            },
                        );
                    }
                });
            }
            dashboard.policies().finish(result);
            policy_edited.set(true);
        }
        let policy_changed = policy_edited.replace(false);
        for update in fingerprint_receiver.try_iter() {
            let rename_result = match &update {
                ipc::fingerprint_update::FingerprintUpdate::Renamed { result, .. } => Some(result.is_ok()),
                _ => None,
            };
            let captured = matches!(&update, ipc::fingerprint_update::FingerprintUpdate::Captured(Ok(_)));
            dashboard.apply_fingerprint_update(update);
            if captured {
                *fingerprint_focus.borrow_mut() = Some("enrollment-name".to_owned());
            } else if let Some(saved) = rename_result {
                *fingerprint_focus.borrow_mut() = if saved {
                    dashboard.selected_fingerprint_row_id()
                } else {
                    Some("fingerprint-inline-name".to_owned())
                };
            }
            fingerprints_changed.set(true);
        }
        let form_changed = fingerprints_changed.replace(false);
        if let Ok(message) = device_receiver.try_recv() {
            dashboard.finish_device_action(message);
            inventory_changed.set(true);
        }
        let device_changed = inventory_changed.replace(false);
        let editor = dashboard.is_editor(current_page);
        if let Some(target) = fingerprint_focus.borrow_mut().take()
            && current_page == Page::Fingerprints
            && (!editor || target == "enrollment-name")
        {
            control.with_runtime(move |runtime| account_menu_scroll::focus(runtime, &target));
            control.request_frame();
        }
        if current_page != rendered_page
            || rendered_editor != editor
            || Some(compact) != rendered_compact
        {
            back_swipe.borrow_mut().cancel();
        }
        if current_page != rendered_page
            || Some(compact) != rendered_compact
            || motion != rendered_motion
            || update.is_some()
            || policy_changed
            || device_changed
            || form_changed
            || rendered_editor != editor
        {
            if let Some(update) = update {
                snapshot = Some(update);
            }
            let menu = dashboard.policies().account_menu_trigger();
            if current_page == Page::Policies && menu.is_some() && menu != account_focus {
                control
                    .with_runtime(|runtime| account_menu_scroll::focus(runtime, "select-search"));
                control.request_frame();
            }
            account_focus = menu;
            let backwards = current_page == Page::Sections
                || (current_page == rendered_page && rendered_editor && !editor);
            let key = (current_page as u64) * 2 + u64::from(editor);
            screens.update(
                key,
                dashboard.view(current_page, snapshot.as_ref(), compact, motion),
                motion && Some(compact) == rendered_compact,
                backwards,
            );
            rendered_editor = editor;
            rendered_motion = motion;
            rendered_compact = Some(compact);
            rendered_page = current_page;
        }
        if current_page == Page::Fingerprints && editor {
            if enrollment_viewport != Some(frame.viewport) || form_changed {
                enrollment_viewport = Some(frame.viewport);
                let dashboard = Rc::clone(&dashboard);
                let changed = Rc::clone(&fingerprints_changed);
                control.with_runtime(move |runtime| {
                    if dashboard.fit_fingerprint_enrollment(runtime) {
                        changed.set(true);
                        runtime.request_frame(obscur_kit::runtime::frame::FrameReason::UiCommit);
                    }
                });
            }
        } else {
            enrollment_viewport = None;
        }
        if let Some(offset) = back_swipe.borrow().offset().filter(|_| motion) {
            screens.track_back(offset, frame.viewport.width, compact, || {
                let parent = if editor { current_page } else { Page::Sections };
                let view =
                    dashboard.view_with_editor(parent, snapshot.as_ref(), compact, false, false);
                ((parent as u64) * 2, view)
            });
        } else {
            screens.release_back();
        }
        let state = control.window_state();
        let maximized = state.is_some_and(|state| state.maximized || state.fullscreen);
        screens.set_window_maximized(maximized);
        screens.set_window_controls(window_controls::view(
            &theme.snapshot(),
            maximized,
            frame.scale.ceil().max(1.0) as u16,
            &chrome_localizer,
        ));
        screens.view(frame.viewport.width, compact, &control)
    })
    .run()?;
    Ok(())
}
