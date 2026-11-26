use carosign_protocol::carosign::common::v1 as common;

/// Selects the available verification-enabled device from the service response.
pub fn select_authenticator(entries: &[common::AuthenticatorDescriptor]) -> Option<Vec<u8>> {
    entries.iter().find_map(|entry| {
        let valid_id = entry.authenticator_id.len() == 16
            && entry.authenticator_id.iter().any(|byte| *byte != 0);
        let can_verify = entry.capabilities.as_ref().is_some_and(|capabilities| {
            capabilities
                .operations
                .contains(&(common::ProviderOperation::ProviderVerify as i32))
        });
        (entry.available && valid_id && can_verify).then(|| entry.authenticator_id.clone())
    })
}

pub(super) fn select_for_account(
    entries: &[common::AuthenticatorDescriptor],
    mut has_enrollment: impl FnMut(
        &[u8],
    ) -> Result<
        bool,
        carosign_linux_ipc::client::transport::TransportError,
    >,
) -> Result<Option<Vec<u8>>, carosign_linux_ipc::client::transport::TransportError> {
    let candidates: Vec<_> = entries
        .iter()
        .filter_map(|entry| select_authenticator(std::slice::from_ref(entry)))
        .collect();
    if candidates.len() <= 1 {
        return Ok(candidates.into_iter().next());
    }
    let mut selected = None;
    for id in candidates {
        if has_enrollment(&id)? {
            if selected.is_some() {
                return Ok(None);
            }
            selected = Some(id);
        }
    }
    Ok(selected)
}

#[cfg(test)]
#[path = "selection.tests.rs"]
mod tests;
