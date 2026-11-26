use carosign_core::ids::OperationId;
use carosign_persistence::model::MutationPhase;
use carosign_protocol::carosign::client::v1::CancelReceiptKind;
use carosign_runtime::ports::{AuthorityRefusal, CallerContext};

use super::operation_status::OperationStatus;

pub(super) fn cancel(
    status: &OperationStatus<'_>,
    caller: &CallerContext,
    operation: OperationId,
) -> Result<CancelReceiptKind, AuthorityRefusal> {
    if let Some(registry) = status.directory.registry(operation)
        && let Some((subject, owner, target)) = registry.cancellation_for(operation, caller)
    {
        status.policy.authorize_cancel(caller.uid(), Some(subject), owner)?;
        if registry.live_status(operation).is_some_and(|live| live.phase.is_terminal()) {
            return Ok(CancelReceiptKind::CancelReceiptAlreadyTerminal);
        }
        if let Some(target) = target {
            return match target.request() {
                Ok(()) => Ok(CancelReceiptKind::CancelReceiptRequested),
                Err(()) if registry.live_status(operation).is_some_and(|live| live.phase.is_terminal()) => {
                    Ok(CancelReceiptKind::CancelReceiptAlreadyTerminal)
                }
                Err(()) => Err(AuthorityRefusal::Unavailable),
            };
        }
        return Err(AuthorityRefusal::Unavailable);
    }

    status.policy.authorize_cancel(caller.uid(), None, false)?;
    let record = carosign_persistence::mutation_store::read_mutation_by_operation_v4(status.store, operation)
        .map_err(|_| AuthorityRefusal::Unavailable)?
        .ok_or(AuthorityRefusal::Denied)?;
    status.policy.authorize_cancel(caller.uid(), Some(record.subject), false)?;
    if record.phase == MutationPhase::Completed {
        Ok(CancelReceiptKind::CancelReceiptAlreadyTerminal)
    } else {
        Err(AuthorityRefusal::Unavailable)
    }
}
