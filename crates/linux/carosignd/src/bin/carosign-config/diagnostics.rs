use std::{
    error::Error,
    io::{self, Write},
    path::Path,
};

use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::client::v1::{
    GetHealth, ListAuthenticators, client_frame::Payload,
};

use super::Configuration;

pub(super) fn diagnose(configuration: &Configuration<'_>) -> Result<(), Box<dyn Error>> {
    let mut output = io::stdout().lock();
    let config = configuration.load()?;
    let mut failures = 0;
    writeln!(output, "OK {}", configuration.service.display())?;
    let (kind, path) = match config.provider_profile.as_deref() {
        Some(path) => ("profile", Path::new(path)),
        None => (
            "providers",
            Path::new(carosignd::config::DEFAULT_PROVIDER_ALLOWLIST),
        ),
    };
    for (kind, path) in [("policy", Path::new(&config.policy)), (kind, path)] {
        if kind == "providers" && !path.try_exists()? {
            writeln!(output, "No provider catalog at {}", path.display())?;
            continue;
        }
        match configuration.check(kind, path) {
            Ok(()) => writeln!(output, "OK {}", path.display())?,
            Err(error) => {
                eprintln!("{}: {error}", path.display());
                failures += 1;
            }
        }
    }
    let transport = Transport::connect_root(&config.socket)
        .map_err(|error| format!("Cannot connect to {}: {error:?}", config.socket))?;
    match transport
        .request(Payload::GetHealth(GetHealth {}))
        .map_err(|error| format!("{error:?}"))?
        .payload
    {
        Some(Payload::Health(health)) => {
            writeln!(
                output,
                "service_ready={} available={} unavailable={} reconciliation={}",
                health.service_ready,
                health.available_authenticators,
                health.unavailable_authenticators,
                health.scopes_needing_reconciliation
            )?;
            failures += usize::from(!health.service_ready);
        }
        Some(Payload::ProtocolError(error)) => {
            eprintln!(
                "Health request refused: {:?}",
                error.error.map(|error| error.code())
            );
            failures += 1;
        }
        _ => return Err("Unexpected health response".into()),
    }
    match transport
        .request(Payload::ListAuthenticators(ListAuthenticators {}))
        .map_err(|error| format!("{error:?}"))?
        .payload
    {
        Some(Payload::Authenticators(inventory)) => {
            if let Some(providers) = inventory.providers {
                for provider in providers.publications {
                    writeln!(
                        output,
                        "provider={} reachable={} devices={}",
                        provider.name,
                        provider.reachable,
                        provider.authenticator_ids.len()
                    )?;
                }
            }
            for device in inventory.entries {
                let id: String = device
                    .authenticator_id
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                writeln!(
                    output,
                    "device={id} available={} epoch={} reason={}",
                    device.available,
                    device.device_epoch,
                    device.unavailability_reason().as_str_name()
                )?;
            }
        }
        Some(Payload::ProtocolError(error)) => {
            eprintln!(
                "Inventory request refused: {:?}",
                error.error.map(|error| error.code())
            );
            failures += 1;
        }
        _ => return Err("Unexpected inventory response".into()),
    }
    if failures == 0 {
        Ok(())
    } else {
        Err("Diagnostics reported failures".into())
    }
}
