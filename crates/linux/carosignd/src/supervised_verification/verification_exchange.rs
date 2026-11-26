use super::{
    CleanupScope, Duration, Error, Instant, LocalResourceOwner, Outcome, PermittedSet,
    ProviderFrame, ProviderSupervisor, QuiesceAssessment, ResourceLease, ResourcePort,
    VerificationExchange, VerificationMessage, prepare_tunnel_until, remaining,
};

/// Performs one inspection with this issue of `resources` and the general term.
/// Before the call, the politician is obliged to authorize the transaction and choose the
/// provider, and at the end of the process stops; the issuance is released only after that.
#[allow(clippy::too_many_arguments)]
pub async fn verify(
    supervisor: &mut ProviderSupervisor,
    resources: &LocalResourceOwner,
    lease: ResourceLease,
    package_id: [u8; 16],
    profile_id: [u8; 16],
    frame: ProviderFrame,
    permitted: PermittedSet,
    timeout: Duration,
) -> Result<Outcome, Error> {
    let result = match Instant::now().checked_add(timeout) {
        Some(deadline) => exchange(
            supervisor, resources, &lease, package_id, profile_id, frame, permitted, deadline,
        ),
        None => Err(Error::Timeout),
    };
    // Even an exchange error does not eliminate the need to stop FD holders.
    let stopped = supervisor.stop_and_confirm(resources).is_ok();
    let released = resources.release(lease).await;
    if !stopped || released != QuiesceAssessment::Quiesced {
        return Err(Error::Cleanup);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn exchange(
    supervisor: &mut ProviderSupervisor,
    resources: &LocalResourceOwner,
    lease: &ResourceLease,
    package_id: [u8; 16],
    profile_id: [u8; 16],
    frame: ProviderFrame,
    permitted: PermittedSet,
    deadline: Instant,
) -> Result<Outcome, Error> {
    let owner = lease.owner();
    if owner.worker != supervisor.worker_id()
        || frame.worker_instance_id != owner.worker.as_bytes()
        || frame.operation_id != owner.operation.as_bytes()
        || frame.device_epoch != owner.authenticator.device_epoch.value()
        || package_id.iter().all(|b| *b == 0)
        || profile_id.iter().all(|b| *b == 0)
        || !resources.validate_live(lease)
    {
        return Err(Error::Context);
    }
    let lease_scope = CleanupScope::of(lease);
    let (channel, _, _) = prepare_tunnel_until(
        supervisor,
        resources,
        &lease_scope,
        package_id,
        profile_id,
        deadline,
        false,
    )?;
    let mut exchange =
        VerificationExchange::start_channel(channel, frame, permitted, remaining(deadline)?)
            .map_err(|_| Error::Protocol)?;
    loop {
        if !resources.validate_live(lease) {
            return Err(Error::Context);
        }
        match exchange
            .next(remaining(deadline)?)
            .map_err(|_| Error::Protocol)?
        {
            VerificationMessage::Accepted | VerificationMessage::Progress(_) => {}
            VerificationMessage::Rejected(error) => return Ok(Outcome::Rejected(error)),
            VerificationMessage::Outcome { observation } => {
                return Ok(Outcome::Observation(observation));
            }
        }
    }
}
