use carosign_protocol::carosign::client::v1 as wire;
use carosignctl::client::ClientError;

pub(super) fn format_snapshot(
    snapshot: wire::OperationSnapshot,
    expected: [u8; 16],
) -> Result<Vec<String>, ClientError> {
    use wire::OperationPhase;
    if snapshot.operation_id != expected {
        return Err(ClientError::Unexpected);
    }
    let phase = match OperationPhase::from_i32(snapshot.phase) {
        Some(OperationPhase::OpQueued) => "в очереди",
        Some(OperationPhase::OpWaitingForUser) => "ожидает пользователя",
        Some(OperationPhase::OpRunning) => "выполняется",
        Some(OperationPhase::OpCancelling) => "отменяется",
        Some(OperationPhase::OpFinalizing) => "завершается",
        Some(OperationPhase::OpTerminal) => "завершена",
        Some(OperationPhase::OpNeedsReconciliation) => "нужна сверка",
        _ => return Err(ClientError::Unexpected),
    };
    let terminal = snapshot.terminal.as_ref();
    if matches!(
        OperationPhase::from_i32(snapshot.phase),
        Some(OperationPhase::OpTerminal)
    ) != terminal.is_some()
    {
        return Err(ClientError::Unexpected);
    }
    let mut lines = vec![format!(
        "операция: {phase}; последнее событие: {}",
        snapshot.last_event_sequence
    )];
    if let Some(terminal) = terminal {
        let outcome = match &terminal.kind {
            Some(wire::terminal_result::Kind::Verification(result)) => {
                match wire::VerificationResult::from_i32(result.result) {
                    Some(wire::VerificationResult::VerifyFactorAccepted) => "фактор подтверждён",
                    Some(wire::VerificationResult::VerifyNotMatched) => "не совпало",
                    Some(wire::VerificationResult::VerifyNoSample) => "образец не получен",
                    Some(wire::VerificationResult::VerifyCancelled) => "отменено",
                    Some(wire::VerificationResult::VerifyUnavailable) => "недоступно",
                    Some(wire::VerificationResult::VerifyFailed) => "ошибка",
                    _ => return Err(ClientError::Unexpected),
                }
            }
            Some(wire::terminal_result::Kind::Mutation(result)) => {
                match wire::MutationResult::from_i32(result.result) {
                    Some(wire::MutationResult::MutationCommitted) => "изменение сохранено",
                    Some(wire::MutationResult::MutationNoChange) => "изменений нет",
                    Some(wire::MutationResult::MutationNeedsReconciliation) => "нужна сверка",
                    Some(wire::MutationResult::MutationFailed) => "ошибка",
                    _ => return Err(ClientError::Unexpected),
                }
            }
            None => return Err(ClientError::Unexpected),
        };
        lines.push(format!("результат: {outcome}"));
    }
    Ok(lines)
}
