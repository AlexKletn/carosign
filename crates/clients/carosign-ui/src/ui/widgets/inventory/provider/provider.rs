use obscur_kit::view::view::ViewFragment;
use obscur_kit::{OkuiScreen, ScreenData, ScreenError};

use crate::ui::shared;

pub(in crate::ui::widgets::inventory) fn template() -> Result<OkuiScreen, ScreenError> {
    let data = ScreenData::new();
    data.set("status_class", "status");
    OkuiScreen::compile_with(include_str!("provider.okui"), data)
}

pub(in crate::ui::widgets::inventory) fn render(
    template: &OkuiScreen,
    title: &str,
    status: &str,
    class: &str,
    devices: ViewFragment,
) -> ViewFragment {
    let data = template.data();
    data.set("name", title);
    data.set("status", status);
    data.set("status_class", class);
    shared::fill(template.view(), [("devices", devices)]).into_fragment()
}
