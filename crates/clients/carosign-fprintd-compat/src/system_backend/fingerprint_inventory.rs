use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::{client::v1 as wire, common::v1 as common};

pub(super) fn authenticator(transport: &Transport, selected: Option<&[u8]>) -> Option<Vec<u8>> {
    authenticator_for(
        transport,
        selected,
        common::ProviderOperation::ProviderVerify,
    )
}

pub(super) fn authenticator_for(
    transport: &Transport,
    selected: Option<&[u8]>,
    operation: common::ProviderOperation,
) -> Option<Vec<u8>> {
    select_fingerprint(transport, selected, operation, |entry| entry.available)
        .map(|entry| entry.authenticator_id)
}

pub(super) fn listed_fingerprint(
    transport: &Transport,
    selected: Option<&[u8]>,
) -> Option<Vec<u8>> {
    select_fingerprint(
        transport,
        selected,
        common::ProviderOperation::ProviderVerify,
        |_| true,
    )
    .map(|entry| entry.authenticator_id)
}

pub(super) fn enrollment_stages(transport: &Transport, selected: Option<&[u8]>) -> Option<u32> {
    select_fingerprint(
        transport,
        selected,
        common::ProviderOperation::ProviderEnroll,
        |entry| entry.available,
    )?
    .capabilities?
    .enrollment_stages
    .filter(|stages| (1..=super::enrollment_progress::MAX_STAGES).contains(stages))
}

fn select_fingerprint(
    transport: &Transport,
    selected: Option<&[u8]>,
    operation: common::ProviderOperation,
    usable: impl Fn(&common::AuthenticatorDescriptor) -> bool,
) -> Option<common::AuthenticatorDescriptor> {
    fingerprints(transport)?
        .into_iter()
        .filter(|entry| {
            selected.is_none_or(|id| entry.authenticator_id == id)
                && entry
                    .capabilities
                    .as_ref()
                    .is_some_and(|value| value.operations.contains(&(operation as i32)))
                && usable(entry)
        })
        .min_by_key(|entry| !entry.available)
}

pub(crate) fn fingerprints(transport: &Transport) -> Option<Vec<common::AuthenticatorDescriptor>> {
    let response = match transport.request(wire::client_frame::Payload::ListAuthenticators(
        wire::ListAuthenticators {},
    )) {
        Ok(response) => response,
        Err(_) => {
            crate::diagnostics::event("ipc_list_error");
            return None;
        }
    };
    let Some(wire::client_frame::Payload::Authenticators(list)) = response.payload else {
        crate::diagnostics::event("ipc_list_invalid_reply");
        return None;
    };
    Some(
        list.entries
            .into_iter()
            .filter(|entry| {
                let provider_unreachable = list.providers.as_ref().is_some_and(|providers| {
                    providers.publications.iter().any(|provider| {
                        !provider.reachable
                            && provider.authenticator_ids.contains(&entry.authenticator_id)
                    })
                });
                (entry.available || !provider_unreachable)
                    && entry.authenticator_id.len() == 16
                    && entry.authenticator_id.iter().any(|value| *value != 0)
                    && entry
                        .capabilities
                        .as_ref()
                        .is_some_and(|value| value.modality() == common::Modality::Fingerprint)
            })
            .collect(),
    )
}
