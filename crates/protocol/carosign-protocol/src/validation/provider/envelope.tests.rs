use super::{check_provider_envelope, limits, provider};
use provider::{
    EnrollmentTransactionAction, EnrollmentTransactionRequest, EnrollmentTransactionResult,
    EnrollmentTransactionState, ProviderFrame, provider_frame::Payload,
};

fn transaction_frame(payload: Payload) -> ProviderFrame {
    ProviderFrame {
        major: limits::MAJOR,
        minor: limits::MINOR_BASELINE,
        frame_seq: 1,
        worker_instance_id: vec![1; 16],
        command_id: vec![2; 16],
        operation_id: vec![3; 16],
        device_epoch: 1,
        uid: None,
        payload: Some(payload),
    }
}

#[test]
fn rollback_request_requires_distinct_command_and_valid_mutation() {
    let request = EnrollmentTransactionRequest {
        mutation_id: vec![4; 16],
        enrollment_command_id: vec![5; 16],
        action: EnrollmentTransactionAction::EnrollmentTransactionRollback as i32,
    };
    let mut frame = transaction_frame(Payload::EnrollmentTransactionRequest(request.clone()));
    assert!(check_provider_envelope(&frame).is_ok());

    frame.command_id = request.enrollment_command_id.clone();
    assert!(check_provider_envelope(&frame).is_err());

    frame.command_id = vec![2; 16];
    frame.payload = Some(Payload::EnrollmentTransactionRequest(
        EnrollmentTransactionRequest {
            mutation_id: vec![0; 16],
            ..request
        },
    ));
    assert!(check_provider_envelope(&frame).is_err());
}

#[test]
fn transaction_result_requires_explicit_state_and_operation() {
    let mut result = EnrollmentTransactionResult {
        mutation_id: vec![4; 16],
        enrollment_command_id: vec![5; 16],
        state: EnrollmentTransactionState::EnrollmentTransactionRolledBack as i32,
    };
    let mut frame = transaction_frame(Payload::EnrollmentTransactionResult(result.clone()));
    assert!(check_provider_envelope(&frame).is_ok());
    frame.operation_id.clear();
    assert!(check_provider_envelope(&frame).is_err());

    result.state = 0;
    let frame = transaction_frame(Payload::EnrollmentTransactionResult(result));
    assert!(check_provider_envelope(&frame).is_err());
}
