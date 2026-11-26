use carosign_l10n::Localizer;
use obscur_kit::{
    App, AppControl, WindowAction,
    identity::InternedSymbol,
    interaction::{
        event::UiEvent,
        pointer::{PointerButton, PointerEventKind},
    },
    system::appearance::{SystemAppearance, WindowButton},
    system_platform::icons::window_button_icon_name,
    ui_ir::name_binding::{TagPosition, attr_symbol, component_id_for},
    view::view::{
        EVENT_CLICK, View, ViewEventBinding, ViewFragment, ViewNode, ViewProperty, ViewValue,
    },
};

pub(super) fn bind(
    app: App,
    control: &AppControl,
    dismiss_overlay: impl Fn() -> bool + 'static,
) -> App {
    let mover = control.clone();
    let app = app.on_command(
        InternedSymbol::named("window_move"),
        move |event: &mut UiEvent| {
            if !dismiss_overlay() && is_window_drag_press(event) {
                mover.request_window_action(WindowAction::BeginMove);
            }
        },
    );
    [
        ("window_minimize", WindowAction::Minimize),
        ("window_maximize", WindowAction::ToggleMaximized),
        ("window_close", WindowAction::Close),
    ]
    .into_iter()
    .fold(app, |app, (name, action)| {
        let control = control.clone();
        app.on_command(InternedSymbol::named(name), move || {
            control.request_window_action(action)
        })
    })
}

fn is_window_drag_press(event: &UiEvent) -> bool {
    event.pointer().is_some_and(|pointer| {
        pointer.kind() == PointerEventKind::Down
            && (pointer.device().is_touchscreen()
                || pointer.button() == Some(PointerButton::Primary))
    })
}

pub(super) fn view(
    appearance: &SystemAppearance,
    maximized: bool,
    scale: u16,
    localizer: &Localizer,
) -> WindowControls {
    let decoration = appearance.window_decoration.as_ref();
    let layout = decoration.map(|decoration| &decoration.button_layout);
    let icon_source = |button, fallback: &str| {
        let name = window_button_icon_name(decoration, button, maximized);
        match appearance.icon_theme.as_deref().zip(name) {
            Some((theme, name)) => {
                format!("window-theme-icon/{scale}/{theme}/{name}/window-{fallback}.svg")
            }
            None => format!("window-{fallback}.svg"),
        }
    };
    let left = layout
        .and_then(|layout| layout.left.as_deref())
        .unwrap_or_default();
    let right = layout
        .and_then(|layout| layout.right.as_deref())
        .unwrap_or(&[
            WindowButton::Minimize,
            WindowButton::Maximize,
            WindowButton::Close,
        ]);
    let (left, left_width) = group(
        left,
        "window-controls window-controls-left",
        maximized,
        localizer,
        &icon_source,
    );
    let (right, right_width) = group(
        right,
        "window-controls window-controls-right",
        maximized,
        localizer,
        &icon_source,
    );
    WindowControls {
        view: View::from_fragment(ViewFragment::from_nodes(vec![left, right])),
        left_width,
        right_width,
    }
}

pub(super) struct WindowControls {
    pub(super) view: View,
    pub(super) left_width: usize,
    pub(super) right_width: usize,
}

fn group(
    buttons: &[WindowButton],
    class: &str,
    maximized: bool,
    localizer: &Localizer,
    icon_source: &impl Fn(WindowButton, &str) -> String,
) -> (ViewNode, usize) {
    let mut width = 0;
    let buttons: Vec<_> = buttons
        .iter()
        .filter_map(|button| {
            let (command, label, icon) = match button {
                WindowButton::Minimize => ("window_minimize", "ui-window-minimize", "minimize"),
                WindowButton::Maximize if maximized => {
                    ("window_maximize", "ui-window-restore", "restore")
                }
                WindowButton::Maximize => ("window_maximize", "ui-window-maximize", "maximize"),
                WindowButton::Close => ("window_close", "ui-window-close", "close"),
                WindowButton::Spacer => {
                    width += 8;
                    return Some(element("div", "window-controls-spacer"));
                }
                _ => return None,
            };
            width += 28;
            Some(
                element("button", "window-control")
                    .with_prop(ViewProperty::new(
                        attr_symbol("aria-label"),
                        ViewValue::Text(localizer.text(label).into()),
                    ))
                    .with_event(ViewEventBinding::new(
                        EVENT_CLICK,
                        InternedSymbol::named(command),
                    ))
                    .with_children(ViewFragment::from_nodes(vec![image(
                        "window-icon",
                        icon_source(*button, icon),
                    )])),
            )
        })
        .collect();
    width += 3 * buttons.len().saturating_sub(1);
    (
        element("div", class)
            .style(format!("width: {width}px;"))
            .with_children(ViewFragment::from_nodes(buttons)),
        width,
    )
}

fn image(class: &str, source: String) -> ViewNode {
    element("img", class)
        .with_prop(ViewProperty::new(attr_symbol("src"), source.into()))
        .with_prop(ViewProperty::new(attr_symbol("alt"), "".into()))
}

fn element(tag: &str, class: &str) -> ViewNode {
    ViewNode::element(component_id_for(tag, TagPosition::Element)).class(class)
}

#[cfg(test)]
#[path = "window_controls.tests.rs"]
mod tests;
