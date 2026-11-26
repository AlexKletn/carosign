use carosign_l10n::Localizer;
use obscur_kit::view::view::{View, ViewFragment};
use obscur_kit::{OkuiScreen, ScreenData, ScreenError};

use crate::domain::QueryError;
use crate::domain::Snapshot;
use crate::ui::{shared, widgets::Inventory};

pub(in crate::ui) struct Platform {
    layout: OkuiScreen,
    service: OkuiScreen,
    inventory: Inventory,
}

impl Platform {
    pub(in crate::ui) fn begin_device_action(&self, id: &str) -> bool {
        self.inventory.begin_action(id)
    }

    pub(in crate::ui) fn finish_device_action(&self, message: &'static str) {
        self.inventory.finish_action(message);
    }

    pub(in crate::ui) fn toggle_device(&self, id: &str) {
        self.inventory.toggle_device(id);
    }

    pub(in crate::ui) fn new(l10n: &Localizer) -> Result<Self, ScreenError> {
        let data = ScreenData::new();
        data.set("state_class", "status platform-service-status");
        data.set("counts_class", "platform-counts-hidden");
        data.set("recovery_class", "platform-hidden");
        data.set("restart_status_class", "platform-hidden");
        data.set("restart_disabled", false);
        for (name, key) in [
            ("state_label", "ui-platform-service"),
            ("available_label", "monitor-ready-devices"),
            ("unavailable_label", "monitor-unavailable-devices"),
            ("reconciliation_label", "monitor-reconciliation"),
            ("restart_label", "ui-policy-restart-service"),
        ] {
            data.set(name, l10n.text(key));
        }
        Ok(Self {
            layout: OkuiScreen::compile(include_str!("platform.okui"))?,
            service: OkuiScreen::compile_with(include_str!("platform_service.okui"), data)?,
            inventory: Inventory::new(l10n)?,
        })
    }

    pub(in crate::ui) fn render(
        &self,
        snapshot: Option<&Snapshot>,
        l10n: &Localizer,
        restart: (bool, Option<&str>),
    ) -> View {
        let health = snapshot.map(|snapshot| &snapshot.health);
        let (state, class) = match health {
            None => ("monitor-loading", "status"),
            Some(Ok(health)) if health.service_ready => {
                ("ui-platform-service-running", "status ready")
            }
            Some(Ok(_)) => ("monitor-not-ready", "status warning"),
            Some(Err(error)) => (error.message_key(), "status warning"),
        };
        let data = self.service.data();
        data.set("state", l10n.text(state));
        data.set("state_class", format!("{class} platform-service-status"));
        let can_recover = match health {
            Some(Ok(health)) => !health.service_ready,
            Some(Err(QueryError::Unavailable | QueryError::Unexpected)) => true,
            _ => false,
        };
        data.set(
            "recovery_class",
            if can_recover {
                "platform-recovery"
            } else {
                "platform-hidden"
            },
        );
        data.set("recovery_hint", l10n.text("ui-platform-service-recovery"));
        data.set("restart_disabled", restart.0);
        data.set(
            "restart_status",
            restart.1.map_or_else(String::new, |key| l10n.text(key)),
        );
        data.set(
            "restart_status_class",
            match restart.1 {
                Some("ui-policy-restart-failed") => "status warning platform-status-text",
                Some(_) => "status platform-status-text",
                None => "platform-hidden",
            },
        );
        if let Some(Ok(health)) = health {
            data.set("counts_class", "platform-counts");
            data.set("available", health.available_authenticators.to_string());
            data.set("unavailable", health.unavailable_authenticators.to_string());
            data.set(
                "reconciliation",
                health.scopes_needing_reconciliation.to_string(),
            );
        } else {
            data.set("counts_class", "platform-counts-hidden");
            for count in ["available", "unavailable", "reconciliation"] {
                data.set(count, "");
            }
        }
        let inventory = snapshot
            .filter(|snapshot| {
                !matches!(
                    (&snapshot.health, &snapshot.inventory),
                    (Err(health), Err(inventory)) if health == inventory
                )
            })
            .map(|snapshot| self.inventory.render(snapshot, l10n).into_fragment())
            .unwrap_or_default();
        let service = self.service.view().into_fragment();
        let has_inventory = !inventory.nodes().is_empty();
        let sections = if has_inventory {
            [inventory, service]
        } else {
            [service, inventory]
        };
        let content = sections
            .into_iter()
            .flat_map(|section| section.into_nodes())
            .collect();
        shared::fill(
            self.layout.view(),
            [("sections", ViewFragment::from_nodes(content))],
        )
    }
}
