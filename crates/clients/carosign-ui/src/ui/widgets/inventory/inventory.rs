use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};

use carosign_l10n::Localizer;
use carosign_protocol::carosign::client::v1::Authenticators;
use obscur_kit::view::view::{View, ViewFragment};
use obscur_kit::{OkuiScreen, ScreenData, ScreenError};

use crate::domain::Snapshot;

use crate::ui::shared;

use super::{device, message, provider};

pub(in crate::ui) struct Inventory {
    layout: OkuiScreen,
    provider: OkuiScreen,
    device: OkuiScreen,
    message: OkuiScreen,
    expanded: RefCell<Option<String>>,
    action: RefCell<Option<(String, &'static str)>>,
    action_pending: Cell<bool>,
}

impl Inventory {
    pub(in crate::ui) fn new(l10n: &Localizer) -> Result<Self, ScreenError> {
        let data = ScreenData::new();
        data.set("heading", l10n.text("ui-platform-devices"));
        Ok(Self {
            layout: OkuiScreen::compile_with(include_str!("inventory.okui"), data)?,
            provider: provider::template()?,
            device: device::template()?,
            message: message::template()?,
            expanded: RefCell::new(None),
            action: RefCell::new(None),
            action_pending: Cell::new(false),
        })
    }

    pub(in crate::ui) fn toggle_device(&self, id: &str) {
        let mut expanded = self.expanded.borrow_mut();
        *expanded = (expanded.as_deref() != Some(id)).then(|| id.to_owned());
    }

    pub(in crate::ui) fn begin_action(&self, id: &str) -> bool {
        if self.expanded.borrow().as_deref() != Some(id) || self.action_pending.get() {
            return false;
        }
        self.action_pending.set(true);
        *self.action.borrow_mut() = Some((id.to_owned(), "ui-device-action-pending"));
        true
    }

    pub(in crate::ui) fn finish_action(&self, message: &'static str) {
        self.action_pending.set(false);
        if let Some((_, status)) = self.action.borrow_mut().as_mut() {
            *status = message;
        }
    }

    pub(in crate::ui) fn render(&self, snapshot: &Snapshot, l10n: &Localizer) -> View {
        let cards = match &snapshot.inventory {
            Ok(inventory) => self.providers(inventory, l10n),
            Err(error) => message::render(&self.message, &l10n.text(error.message_key())),
        };
        shared::fill(self.layout.view(), [("providers", cards)])
    }

    fn providers(&self, inventory: &Authenticators, l10n: &Localizer) -> ViewFragment {
        let expanded = self.expanded.borrow();
        let action = self.action.borrow();
        let render_device = |entry, managed: bool| {
            let feedback = action.as_ref().map(|(id, status)| (id.as_str(), *status));
            device::render(
                entry,
                l10n,
                &self.device,
                expanded.as_deref(),
                managed.then_some((self.action_pending.get(), feedback)),
            )
            .into_nodes()
        };
        let mut devices: BTreeMap<_, _> = inventory
            .entries
            .iter()
            .map(|device| (device.authenticator_id.as_slice(), device))
            .collect();
        let mut cards = Vec::new();
        match &inventory.providers {
            Some(providers) if !providers.publications.is_empty() => {
                let mut publications: Vec<_> = providers.publications.iter().collect();
                publications.sort_by(|left, right| left.name.cmp(&right.name));
                for provider in publications {
                    let (status, class) = if provider.reachable {
                        ("monitor-reachable", "status ready")
                    } else {
                        ("monitor-unreachable", "status warning")
                    };
                    let mut rows: Vec<_> = provider
                        .authenticator_ids
                        .iter()
                        .filter_map(|id| devices.remove(id.as_slice()))
                        .flat_map(|entry| render_device(entry, true))
                        .collect();
                    if rows.is_empty() {
                        rows = message::render(
                            &self.message,
                            &l10n.text("monitor-no-published-devices"),
                        )
                        .into_nodes();
                    }
                    cards.extend(
                        provider::render(
                            &self.provider,
                            &provider.name,
                            &l10n.text(status),
                            class,
                            ViewFragment::from_nodes(rows),
                        )
                        .into_nodes(),
                    );
                }
            }
            Some(_) => cards.extend(
                message::render(&self.message, &l10n.text("monitor-no-providers")).into_nodes(),
            ),
            None => cards.extend(
                message::render(
                    &self.message,
                    &l10n.text("monitor-provider-info-unavailable"),
                )
                .into_nodes(),
            ),
        }
        if !devices.is_empty() {
            let rows = devices
                .into_values()
                .flat_map(|entry| render_device(entry, false))
                .collect();
            cards.extend(
                provider::render(
                    &self.provider,
                    &l10n.text("monitor-other-devices"),
                    "",
                    "status",
                    ViewFragment::from_nodes(rows),
                )
                .into_nodes(),
            );
        }
        ViewFragment::from_nodes(cards)
    }
}
