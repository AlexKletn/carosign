use std::time::{Duration, Instant};

use carosign_protocol::carosign::broker_relay::v1::Scope;

pub(super) fn revoke_owner(
    held: Option<&Scope>,
    requested: Option<&Scope>,
    requests_pending: bool,
    events_pending: bool,
    inspect: impl FnOnce() -> Result<carosign_protocol::resource_owner::Quiescence, ()>,
) -> Result<carosign_protocol::resource_owner::Quiescence, ()> {
    if held.is_none() || held != requested || requests_pending || events_pending {
        return Err(());
    }
    match inspect()? {
        result @ (carosign_protocol::resource_owner::Quiescence::LeaseRevoked
        | carosign_protocol::resource_owner::Quiescence::ExclusiveLeaseRevoked
        | carosign_protocol::resource_owner::Quiescence::ExclusiveQuiesced) => Ok(result),
        _ => Err(()),
    }
}

pub(super) fn await_owner_revoke(
    owner: &mut crate::owner_channel::OwnerSession,
    first_id: [u8; 16],
) -> Result<carosign_protocol::resource_owner::Quiescence, ()> {
    use carosign_protocol::resource_owner::Quiescence;
    const RETRY_INTERVAL: Duration = Duration::from_millis(10);
    const MAX_WAIT: Duration = Duration::from_secs(5);
    const {
        assert!(
            carosign_protocol::resource_owner::QUIESCENCE_REQUEST_LIMIT as u128
                * RETRY_INTERVAL.as_millis()
                >= MAX_WAIT.as_millis()
        );
    }
    let deadline = Instant::now() + MAX_WAIT;
    let mut id = first_id;
    for _ in 0..carosign_protocol::resource_owner::QUIESCENCE_REQUEST_LIMIT {
        let remaining = deadline.checked_duration_since(Instant::now()).ok_or(())?;
        match owner.inspect_quiescence(id, remaining).map_err(|_| ())? {
            Quiescence::Draining => {
                if deadline.checked_duration_since(Instant::now()).ok_or(())? <= RETRY_INTERVAL {
                    return Err(());
                }
                std::thread::sleep(RETRY_INTERVAL);
                id = carosign_linux::sys::random_16().map_err(|_| ())?;
            }
            result => return Ok(result),
        }
    }
    Err(())
}

#[cfg(test)]
#[path = "owner_revocation.tests.rs"]
mod tests;
