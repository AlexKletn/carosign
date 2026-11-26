use carosign_core::enrollment::EnrollmentRef;

use super::{MutationPlan, PlanError, Scope, Transition, sha256};

/// Canonical record of the Digest Area (Document 33, § Plan and Authority).
///
/// ```text
/// SHA256("bio-mutation-plan-v3" ‖ scope[48] ‖ transition_u32BE
///        ‖ old_target_or_zeroes[40]        // id[16] ‖ auth[16] ‖ gen_u64BE
///        ‖ new_id_or_zeros[16] ‖ new_generation_u64BE
///        ‖ snapshot_count_u32BE ‖ entries)
/// entry = enrollment_id[16] ‖ authenticator_id[16] ‖ generation_u64BE
///         ‖ authority_id[16] ‖ subject_id[16] ‖ binding_kind_u32BE
///         ‖ provider_ref_digest[32]
///         ‖ format_id_len_u16BE ‖ format_id_utf8 ‖ format_version_u32BE
/// ```
pub fn canonical_digest(scope: Scope, plan: &MutationPlan) -> Result<[u8; 32], PlanError> {
    let mut bytes = Vec::with_capacity(512);
    bytes.extend_from_slice(b"bio-mutation-plan-v3");
    bytes.extend_from_slice(&scope.canonical());
    bytes.extend_from_slice(&(plan.transition.wire() as u32).to_be_bytes());
    // ⚠️ Old target: 40 bytes or 40 zeros.
    match plan.old_target {
        Some(old) => {
            bytes.extend_from_slice(old.id.as_bytes());
            bytes.extend_from_slice(old.authenticator.as_bytes());
            bytes.extend_from_slice(&old.generation.value().to_be_bytes());
        }
        None => bytes.extend_from_slice(&[0u8; 40]),
    }
    match plan.new_enrollment {
        Some(id) => bytes.extend_from_slice(id.as_bytes()),
        None => bytes.extend_from_slice(&[0u8; 16]),
    }
    bytes.extend_from_slice(&plan.new_generation.to_be_bytes());
    bytes.extend_from_slice(&(plan.pre_bindings.len() as u32).to_be_bytes());
    for snapshot in &plan.pre_bindings {
        let format = snapshot.format_id.as_bytes();
        if format.len() > 128 {
            return Err(PlanError::FormatTooLong);
        }
        bytes.extend_from_slice(snapshot.enrollment.id.as_bytes());
        bytes.extend_from_slice(snapshot.enrollment.authenticator.as_bytes());
        bytes.extend_from_slice(&snapshot.enrollment.generation.value().to_be_bytes());
        bytes.extend_from_slice(snapshot.subject.authority.as_bytes());
        bytes.extend_from_slice(snapshot.subject.subject.as_bytes());
        bytes.extend_from_slice(&(snapshot.kind.wire() as u32).to_be_bytes());
        bytes.extend_from_slice(&snapshot.provider_ref_digest);
        bytes.extend_from_slice(&(format.len() as u16).to_be_bytes());
        bytes.extend_from_slice(format);
        bytes.extend_from_slice(&snapshot.format_version.to_be_bytes());
    }
    if let Some(label) = &plan.requested_label {
        if label.len() > 64 || matches!(plan.transition, Transition::Remove) {
            return Err(PlanError::TransitionShape);
        }
        bytes.extend_from_slice(b"bio-enrollment-label-v1");
        bytes.extend_from_slice(&(label.len() as u16).to_be_bytes());
        bytes.extend_from_slice(label.as_bytes());
    }
    Ok(sha256(&bytes))
}

/// Digest of the original client change for idempotence: the new registration number is
/// deliberately not included: it is assigned by the service after the request.
pub fn canonical_request_digest(
    scope: Scope,
    transition: Transition,
    old_target: Option<EnrollmentRef>,
    label: Option<&str>,
) -> Result<[u8; 32], PlanError> {
    match (transition, old_target, label) {
        (Transition::EnrollNew, None, Some(label)) if label.len() <= 64 => {}
        (Transition::EnrollReplace, Some(old), Some(label))
            if old.authenticator == scope.authenticator && label.len() <= 64 => {}
        (Transition::Remove, Some(old), None) if old.authenticator == scope.authenticator => {}
        _ => return Err(PlanError::TransitionShape),
    }
    let mut bytes = Vec::with_capacity(160);
    bytes.extend_from_slice(b"bio-mutation-request-v1");
    bytes.extend_from_slice(&scope.canonical());
    bytes.extend_from_slice(&(transition.wire() as u32).to_be_bytes());
    if let Some(old) = old_target {
        bytes.extend_from_slice(old.id.as_bytes());
        bytes.extend_from_slice(old.authenticator.as_bytes());
        bytes.extend_from_slice(&old.generation.value().to_be_bytes());
    } else {
        bytes.extend_from_slice(&[0; 40]);
    }
    match label {
        Some(label) => {
            bytes.push(1);
            bytes.extend_from_slice(&(label.len() as u16).to_be_bytes());
            bytes.extend_from_slice(label.as_bytes());
        }
        None => bytes.push(0),
    }
    Ok(sha256(&bytes))
}
