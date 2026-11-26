use carosign_l10n::Localizer;
use obscur_kit::identity::NodeId;
use obscur_kit::ui_ir::name_binding::{TagPosition, component_id_for};
use obscur_kit::view::view::{TEST_ID, ViewFragment, ViewNode, ViewValue};
use obscur_kit::{OkuiScreen, ScreenData, ScreenError};
use obscur_kit::{runtime::UiRuntime, view::view::View};

use crate::{
    domain::Snapshot,
    ipc::{fingerprint_job::FingerprintJob, fingerprint_update::FingerprintUpdate},
};

use crate::ui::{
    shared,
    widgets::{Fingerprints, Platform, Policies, Sidebar},
};

use crate::ui::page::Page;

pub(crate) struct Dashboard {
    layout: OkuiScreen,
    platform: Platform,
    fingerprints: Fingerprints,
    policies: Policies,
    sidebar: Sidebar,
    localizer: Localizer,
}

impl Dashboard {
    pub(crate) fn new(localizer: Localizer) -> Result<Self, ScreenError> {
        let data = ScreenData::new();
        data.set("page_class", "page");
        data.set("sidebar_class", "navigation-pane");
        data.set("header_class", "header");
        data.set("content_class", "content");
        data.set("content_pane_class", "content-pane");
        data.set("back_class", "header-back-hidden");
        data.set("back_disabled", true);
        data.set("back_label", localizer.text("ui-navigation-back"));
        Ok(Self {
            layout: OkuiScreen::compile_with(include_str!("dashboard.okui"), data)?,
            platform: Platform::new(&localizer)?,
            fingerprints: Fingerprints::new()?,
            policies: Policies::new(&localizer)?,
            sidebar: Sidebar::new(&localizer)?,
            localizer,
        })
    }

    pub(in crate::ui::app) fn enable_fingerprint_demo(&mut self) {
        self.fingerprints.enable_demo(&self.localizer);
    }

    pub(in crate::ui::app) fn fingerprint_command(&self, command: &str) -> bool {
        self.fingerprints
            .enrollment_command(command, &self.localizer)
    }

    pub(in crate::ui::app) fn run_live_fingerprint_command(
        &self,
        command: &str,
    ) -> (bool, Option<FingerprintJob>) {
        self.fingerprints.live_command(command)
    }

    pub(in crate::ui::app) fn apply_fingerprint_update(&self, update: FingerprintUpdate) {
        self.fingerprints.apply_live_update(update);
    }

    pub(in crate::ui::app) fn edit_fingerprint_name(&self, label: &str) -> bool {
        self.fingerprints.edit_enrollment(label)
    }

    pub(in crate::ui::app) fn edit_registered_fingerprint_name(&self, label: &str) -> bool {
        self.fingerprints.edit_registered_name(label)
    }

    pub(in crate::ui::app) fn selected_fingerprint_row_id(&self) -> Option<String> {
        self.fingerprints
            .selected_row_id()
            .map(|id| format!("fingerprint-row-{id}"))
    }

    pub(in crate::ui::app) fn select_registered_fingerprint(
        &self,
        runtime: &UiRuntime,
        target: NodeId,
    ) -> bool {
        Self::target_id(runtime, target, "fingerprint-row-")
            .is_some_and(|id| self.fingerprints.select_registered(id))
    }

    pub(in crate::ui::app) fn toggle_inventory_device(
        &self,
        runtime: &UiRuntime,
        target: NodeId,
    ) -> bool {
        let Some(id) = Self::target_id(runtime, target, "inventory-card-") else {
            return false;
        };
        self.platform.toggle_device(id);
        true
    }

    pub(in crate::ui::app) fn begin_device_action(
        &self,
        runtime: &UiRuntime,
        target: NodeId,
    ) -> Option<(String, bool)> {
        let (id, disable) = Self::target_id(runtime, target, "inventory-disable-")
            .map(|id| (id, true))
            .or_else(|| {
                Self::target_id(runtime, target, "inventory-enable-").map(|id| (id, false))
            })?;
        self.platform
            .begin_device_action(id)
            .then(|| (id.to_owned(), disable))
    }

    pub(in crate::ui::app) fn finish_device_action(&self, message: &'static str) {
        self.platform.finish_device_action(message);
    }

    fn target_id<'a>(runtime: &'a UiRuntime, target: NodeId, prefix: &str) -> Option<&'a str> {
        let mut current = Some(target);
        while let Some(node) = current.and_then(|id| runtime.tree().get(id)) {
            for prop in node.props().iter().filter(|prop| prop.name() == TEST_ID) {
                if let ViewValue::Text(value) = prop.value()
                    && let Some(id) = value.strip_prefix(prefix)
                {
                    return Some(id);
                }
            }
            current = node.logical_parent().parent();
        }
        None
    }

    pub(in crate::ui::app) fn cancel_fingerprint_enrollment(&self) -> bool {
        self.fingerprints.cancel_enrollment()
    }

    pub(in crate::ui::app) fn fit_fingerprint_enrollment(&self, runtime: &UiRuntime) -> bool {
        let nodes = runtime.inspect_snapshot().nodes;
        let bounds = |class: &str| {
            let node = nodes
                .iter()
                .find(|node| node.classes.iter().any(|name| name == class))?;
            runtime.layout_boxes().get(&node.node)
        };
        let (Some(area), Some(form), Some(mark)) = (
            bounds("content-enrollment"),
            bounds("fingerprint-enrollment"),
            bounds("enrollment-mark"),
        ) else {
            return false;
        };
        let center = |bounds: &obscur_kit::layout::model::LayoutBox| {
            bounds.origin().y + bounds.size().height / 2.0
        };
        let mark_center = center(mark);
        let top = form.origin().y;
        let bottom = top + form.size().height;
        let extent = (mark_center - top).max(bottom - mark_center);
        if extent <= 0.0 {
            return false;
        }
        let scale = ((area.size().height / 2.0 - 16.0) / extent).clamp(0.01, 1.0);
        let scaled_center = center(form) + (mark_center - center(form)) * scale;
        self.fingerprints
            .set_enrollment_fit(scale, center(area) - scaled_center)
    }

    pub(in crate::ui::app) fn is_editor(&self, page: Page) -> bool {
        match page {
            Page::Policies => self.policies.is_editing(),
            Page::Fingerprints => self.fingerprints.is_enrolling(),
            _ => false,
        }
    }

    pub(in crate::ui::app) fn policies(&self) -> &Policies {
        &self.policies
    }

    pub(in crate::ui::app) fn can_go_back(&self, page: Page, compact: bool) -> bool {
        self.is_editor(page) || (compact && page != Page::Sections)
    }

    pub(crate) fn view(
        &self,
        page: Page,
        snapshot: Option<&Snapshot>,
        compact: bool,
        motion: bool,
    ) -> View {
        self.view_with_editor(page, snapshot, compact, motion, self.is_editor(page))
    }

    pub(in crate::ui::app) fn view_with_editor(
        &self,
        page: Page,
        snapshot: Option<&Snapshot>,
        compact: bool,
        motion: bool,
        editor: bool,
    ) -> View {
        let editing = page == Page::Policies && editor;
        let sections = page == Page::Sections;
        let can_go_back = editor || (compact && !sections);
        let data = self.layout.data();
        let centered = (page == Page::Policies && self.policies.requires_unlock())
            || (page == Page::Fingerprints && !self.fingerprints.has_entries(snapshot));
        let enrollment = page == Page::Fingerprints && editor;
        let uses_section_band =
            matches!(page, Page::Fingerprints | Page::Policies) && !centered && !enrollment;
        data.set(
            "content_pane_class",
            if page == Page::Platform || uses_section_band {
                "content-pane content-pane-section"
            } else {
                "content-pane"
            },
        );
        data.set(
            "content_class",
            if page == Page::Platform {
                if compact {
                    "content content-platform-compact"
                } else {
                    "content content-platform-wide"
                }
            } else if enrollment {
                "content content-centered content-enrollment"
            } else if centered {
                "content content-centered"
            } else if uses_section_band {
                "content content-section"
            } else {
                "content"
            },
        );
        data.set(
            "page_class",
            if motion {
                "page page-with-motion"
            } else {
                "page"
            },
        );
        data.set(
            "sidebar_class",
            match (compact, sections) {
                (true, true) => "navigation-pane navigation-pane-full",
                (true, false) => "navigation-pane navigation-pane-collapsed",
                (false, _) => "navigation-pane",
            },
        );
        data.set(
            "header_class",
            if sections {
                "content-pane-hidden"
            } else if can_go_back {
                "header header-with-back"
            } else {
                "header"
            },
        );
        data.set(
            "back_class",
            if can_go_back {
                "header-back"
            } else {
                "header-back-hidden"
            },
        );
        data.set("back_disabled", !can_go_back);
        data.set(
            "page_title",
            self.localizer
                .text(if page == Page::Fingerprints && editor {
                    "ui-enroll-title"
                } else if editing {
                    "ui-policy-edit-rule"
                } else {
                    page.label()
                }),
        );
        let content = match page {
            Page::Sections => View::default(),
            Page::Platform => {
                self.platform
                    .render(snapshot, &self.localizer, self.policies.service_restart())
            }
            Page::Fingerprints if editor => {
                self.fingerprints.enrollment_view(&self.localizer, motion)
            }
            Page::Fingerprints => self.fingerprints.render(snapshot, &self.localizer),
            Page::Policies => self.policies.view_with_editor(&self.localizer, editing),
        };
        let content = if uses_section_band {
            let component = component_id_for("div", TagPosition::Element);
            let column = ViewNode::element(component)
                .class("section-column")
                .with_children(content.into_fragment());
            View::single(
                ViewNode::element(component)
                    .class("section-band")
                    .with_children(ViewFragment::from_nodes(vec![column])),
            )
        } else {
            content
        };
        let sidebar = if compact && !sections {
            View::empty()
        } else {
            self.sidebar.view(
                page,
                snapshot,
                &self.localizer,
                self.policies.service_restart(),
            )
        };
        shared::fill(
            self.layout.view(),
            [
                ("sidebar", sidebar.into_fragment()),
                ("sections", content.into_fragment()),
            ],
        )
    }
}

#[path = "dashboard.tests.rs"]
#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "dashboard_render.tests.rs"]
mod dashboard_render_tests;
