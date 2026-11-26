use carosign_l10n::Localizer;
use carosign_protocol::carosign::client::v1 as wire;
use carosignctl::client::ClientError;

pub(super) fn format_authenticators(a: wire::Authenticators) -> Result<Vec<String>, ClientError> {
    if a.entries.len() > carosign_protocol::limits::DEVICES_PER_RESPONSE_MAX {
        return Err(ClientError::Unexpected);
    }
    let localizer = Localizer::from_environment();
    a.entries
        .iter()
        .map(|entry| {
            let id: [u8; 16] = entry
                .authenticator_id
                .as_slice()
                .try_into()
                .map_err(|_| ClientError::Unexpected)?;
            if id.iter().all(|v| *v == 0) {
                return Err(ClientError::Unexpected);
            }
            let mut line = format!(
                "{}  {}",
                carosignctl::output::short(&id),
                if entry.available {
                    "доступен"
                } else {
                    "недоступен"
                }
            );
            if !entry.available
                && let Some(key) = entry.unavailability_reason().message_key()
            {
                line.push_str(" — ");
                line.push_str(&localizer.text(key));
            }
            Ok(line)
        })
        .collect()
}
