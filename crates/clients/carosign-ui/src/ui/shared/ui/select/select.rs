use obscur_kit::identity::InternedSymbol;
use obscur_kit::view::view::{EVENT_CLICK, ViewEventBinding, ViewFragment, ViewNode};
use obscur_kit::{OkuiScreen, ScreenData, ScreenError};

use crate::ui::shared;

pub(in crate::ui) struct Select {
    menu: OkuiScreen,
    option: OkuiScreen,
    trigger: OkuiScreen,
    search: OkuiScreen,
    open: OkuiScreen,
}

impl Select {
    pub(in crate::ui) fn new() -> Result<Self, ScreenError> {
        let open = ScreenData::new();
        open.set("surface_class", "select-open-surface menu-one");
        let menu = ScreenData::new();
        menu.set("options_class", "select-options");
        menu.set("hint_class", "status");
        menu.set("menu_class", "select-menu menu-one");
        let option = ScreenData::new();
        option.set("option_class", "select-menu-option");
        let trigger = ScreenData::new();
        trigger.set("trigger_id", "");
        trigger.set("disabled", true);
        let search = ScreenData::new();
        search.set("trigger_id", "");
        search.set("query", "");
        search.set("search_label", "");
        Ok(Self {
            open: OkuiScreen::compile_with(include_str!("select_open.okui"), open)?,
            menu: OkuiScreen::compile_with(include_str!("select_menu.okui"), menu)?,
            option: OkuiScreen::compile_with(include_str!("select_option.okui"), option)?,
            trigger: OkuiScreen::compile_with(include_str!("select_trigger.okui"), trigger)?,
            search: OkuiScreen::compile_with(include_str!("select_search.okui"), search)?,
        })
    }

    pub(in crate::ui) fn trigger(
        &self,
        id: &str,
        label: String,
        leading: ViewFragment,
        disabled: bool,
        command: InternedSymbol,
    ) -> ViewFragment {
        let data = self.trigger.data();
        data.set("trigger_id", id);
        data.set("label", label);
        data.set("disabled", disabled);
        let nodes = shared::fill(self.trigger.view(), [("leading", leading)])
            .into_fragment()
            .into_nodes()
            .into_iter()
            .map(|node| node.with_event(ViewEventBinding::new(EVENT_CLICK, command)))
            .collect();
        ViewFragment::from_nodes(nodes)
    }

    pub(in crate::ui) fn search(
        &self,
        id: &str,
        query: &str,
        search_label: String,
        leading: ViewFragment,
    ) -> ViewFragment {
        let data = self.search.data();
        data.set("trigger_id", id);
        data.set("query", query);
        data.set("search_label", search_label);
        shared::fill(self.search.view(), [("leading", leading)]).into_fragment()
    }

    pub(in crate::ui) fn option(
        &self,
        label: String,
        leading: ViewFragment,
        active: bool,
        command: InternedSymbol,
    ) -> Vec<ViewNode> {
        let data = self.option.data();
        data.set("label", label);
        data.set(
            "option_class",
            if active {
                "select-menu-option active-option"
            } else {
                "select-menu-option"
            },
        );
        shared::fill(self.option.view(), [("leading", leading)])
            .into_fragment()
            .into_nodes()
            .into_iter()
            .map(|node| node.with_event(ViewEventBinding::new(EVENT_CLICK, command)))
            .collect()
    }

    pub(in crate::ui) fn open(
        &self,
        search: ViewFragment,
        hint: String,
        rows: Vec<ViewNode>,
    ) -> ViewFragment {
        let data = self.menu.data();
        data.set(
            "hint_class",
            if hint.is_empty() {
                "select-empty"
            } else {
                "status"
            },
        );
        data.set(
            "options_class",
            if rows.is_empty() {
                "select-empty"
            } else {
                "select-options"
            },
        );
        data.set("hint", hint);
        let size = match rows.len() {
            0 | 1 => "menu-one",
            2 => "menu-two",
            3 => "menu-three",
            _ => "menu-four",
        };
        data.set("menu_class", format!("select-menu {size}"));
        self.open
            .data()
            .set("surface_class", format!("select-open-surface {size}"));
        let menu = shared::fill(
            self.menu.view(),
            [("options", ViewFragment::from_nodes(rows))],
        )
        .into_fragment();
        shared::fill(self.open.view(), [("search", search), ("menu", menu)]).into_fragment()
    }
}
