use obscur_kit::view::view::ViewFragment;
use obscur_kit::{OkuiScreen, ScreenError};

pub(in crate::ui::widgets::inventory) fn template() -> Result<OkuiScreen, ScreenError> {
    OkuiScreen::compile(include_str!("message.okui"))
}

pub(in crate::ui::widgets::inventory) fn render(
    template: &OkuiScreen,
    message: &str,
) -> ViewFragment {
    template.data().set("message", message);
    template.view().into_fragment()
}
