use carosign_l10n::Localizer;
use carosign_protocol::carosign::common::v1::{
    AuthenticatorDescriptor, Modality, PresentationAttackDetectionSupport, ProviderOperation,
};
use obscur_kit::view::view::ViewFragment;
use obscur_kit::{OkuiScreen, ScreenData, ScreenError};

pub(in crate::ui::widgets::inventory) fn template() -> Result<OkuiScreen, ScreenError> {
    let data = ScreenData::new();
    data.set("status_class", "status");
    data.set("reason_class", "device-detail-hidden");
    data.set("expanded", false);
    data.set("button_id", "");
    data.set("card_id", "");
    data.set("action_id", "");
    data.set("action_class", "device-detail-hidden");
    data.set("action_feedback_class", "device-detail-hidden");
    data.set("action_disabled", false);
    data.set("chevron", "fingerprint-chevron-down.svg");
    data.set("disclosure_class", "device-detail-hidden");
    data.set("icon", "platform.svg");
    data.set("operations_class", "device-detail-hidden");
    data.set("detection_class", "device-detail-hidden");
    OkuiScreen::compile_with(include_str!("device.okui"), data)
}

pub(in crate::ui::widgets::inventory) fn render(
    device: &AuthenticatorDescriptor,
    l10n: &Localizer,
    template: &OkuiScreen,
    expanded_id: Option<&str>,
    action: Option<(bool, Option<(&str, &'static str)>)>,
) -> ViewFragment {
    let data = template.data();
    let capabilities = device.capabilities.as_ref();
    let modality = capabilities.map(|value| value.modality());
    data.set("device", l10n.text(modality_key(modality)));
    data.set(
        "icon",
        if modality == Some(Modality::Fingerprint) {
            "fingerprint.svg"
        } else {
            "platform.svg"
        },
    );
    let (status, class) = if device.available {
        ("monitor-ready", "status ready")
    } else {
        ("monitor-device-unavailable", "status warning")
    };
    data.set("status", l10n.text(status));
    data.set("status_class", class);
    let reason = (!device.available)
        .then(|| device.unavailability_reason().message_key())
        .flatten();
    data.set(
        "reason",
        reason.map_or_else(String::new, |key| l10n.text(key)),
    );
    data.set(
        "reason_class",
        if reason.is_some() {
            "detail device-reason"
        } else {
            "device-detail-hidden"
        },
    );
    let identifier: String = device
        .authenticator_id
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let expanded = expanded_id == Some(identifier.as_str());
    let is_disabled = device.unavailability_reason()
        == carosign_protocol::carosign::common::v1::UnavailabilityReason::DisabledByAdministrator;
    let action_name = if is_disabled { "enable" } else { "disable" };
    let (pending, feedback) = action.unwrap_or_default();
    let feedback = feedback
        .filter(|(id, _)| *id == identifier)
        .map(|(_, key)| key);
    data.set("action_id", format!("inventory-{action_name}-{identifier}"));
    data.set(
        "action_label",
        l10n.text(if is_disabled {
            "ui-device-enable"
        } else {
            "ui-device-disable"
        }),
    );
    data.set("action_disabled", pending);
    data.set(
        "action_class",
        if action.is_some() {
            "device-actions"
        } else {
            "device-detail-hidden"
        },
    );
    data.set(
        "action_feedback",
        feedback.map_or_else(String::new, |key| l10n.text(key)),
    );
    data.set(
        "action_feedback_class",
        match feedback {
            Some("ui-device-action-failed" | "ui-device-action-denied") => "status warning",
            Some(_) => "status",
            None => "device-detail-hidden",
        },
    );
    let button_id = format!("inventory-device-{identifier}");
    data.set("card_id", format!("inventory-card-{identifier}"));
    data.set("identifier", identifier);
    data.set("button_id", button_id.as_str());
    data.set("expanded", expanded);
    data.set(
        "disclosure_class",
        if expanded {
            "device-disclosure"
        } else {
            "device-detail-hidden"
        },
    );
    data.set(
        "chevron",
        if expanded {
            "fingerprint-chevron-up.svg"
        } else {
            "fingerprint-chevron-down.svg"
        },
    );
    let operations = capabilities
        .into_iter()
        .flat_map(|value| &value.operations)
        .filter_map(|value| ProviderOperation::from_i32(*value).and_then(operation_key))
        .map(|key| l10n.text(key))
        .collect::<Vec<_>>();
    data.set("operations", operations.join(" · "));
    data.set(
        "operations_class",
        if operations.is_empty() {
            "device-detail-hidden"
        } else {
            "detail"
        },
    );
    let detection = capabilities
        .and_then(|value| value.assurance.as_ref())
        .and_then(|value| value.presentation_attack_detection.as_ref())
        .map(|detection| match detection.support() {
            PresentationAttackDetectionSupport::Declared => "monitor-pad-declared",
            PresentationAttackDetectionSupport::Unsupported => "monitor-pad-unsupported",
            PresentationAttackDetectionSupport::Unknown => "monitor-pad-unknown",
        });
    data.set(
        "detection",
        detection.map_or_else(String::new, |key| l10n.text(key)),
    );
    data.set(
        "detection_class",
        if detection.is_some() {
            "detail"
        } else {
            "device-detail-hidden"
        },
    );
    ViewFragment::from_nodes(
        template
            .view()
            .into_fragment()
            .into_nodes()
            .into_iter()
            .map(|node| node.with_key(button_id.clone()))
            .collect(),
    )
}

fn modality_key(modality: Option<Modality>) -> &'static str {
    match modality {
        Some(Modality::Fingerprint) => "ui-fingerprint-reader",
        Some(Modality::Face) => "monitor-face",
        Some(Modality::Iris) => "monitor-iris",
        Some(Modality::Voice) => "monitor-voice",
        _ => "monitor-device",
    }
}

fn operation_key(operation: ProviderOperation) -> Option<&'static str> {
    match operation {
        ProviderOperation::Unspecified => None,
        ProviderOperation::ProviderVerify => Some("monitor-verify"),
        ProviderOperation::ProviderEnroll => Some("monitor-enroll"),
        ProviderOperation::ProviderRemove => Some("monitor-remove"),
        ProviderOperation::ProviderEnumerate => Some("monitor-enumerate"),
        ProviderOperation::ProviderQueryMutation => Some("monitor-query-mutation"),
    }
}
