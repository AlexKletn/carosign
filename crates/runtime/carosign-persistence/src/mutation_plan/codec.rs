use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{AuthenticatorId, EnrollmentId, SubjectId};

use super::{
    BindingSnapshot, MutationPlan, PlanCursor, PlanError, SNAPSHOTS_MAX, StoredBindingKind,
    Transition,
};

/// Canonical bytes of a storage plan.
///
/// ⚠️ The order of the fields is as in the canonical record of the digest: so the plan and
/// its digest will not separate during reading.
pub fn encode_plan(plan: &MutationPlan) -> Result<Vec<u8>, PlanError> {
    // Historical binary formats can't keep the v4 label.
    if plan.requested_label.is_some() {
        return Err(PlanError::TransitionShape);
    }
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(&(plan.transition.wire() as u32).to_be_bytes());
    match plan.old_target {
        Some(old) => {
            out.extend_from_slice(old.id.as_bytes());
            out.extend_from_slice(old.authenticator.as_bytes());
            out.extend_from_slice(&old.generation.value().to_be_bytes());
        }
        None => out.extend_from_slice(&[0u8; 40]),
    }
    match plan.new_enrollment {
        Some(id) => out.extend_from_slice(id.as_bytes()),
        None => out.extend_from_slice(&[0u8; 16]),
    }
    out.extend_from_slice(&plan.new_generation.to_be_bytes());
    out.extend_from_slice(&(plan.pre_bindings.len() as u32).to_be_bytes());
    for snapshot in &plan.pre_bindings {
        let format = snapshot.format_id.as_bytes();
        if format.len() > 128 {
            return Err(PlanError::FormatTooLong);
        }
        out.extend_from_slice(snapshot.enrollment.id.as_bytes());
        out.extend_from_slice(snapshot.enrollment.authenticator.as_bytes());
        out.extend_from_slice(&snapshot.enrollment.generation.value().to_be_bytes());
        out.extend_from_slice(snapshot.subject.authority.as_bytes());
        out.extend_from_slice(snapshot.subject.subject.as_bytes());
        out.extend_from_slice(&(snapshot.kind.wire() as u32).to_be_bytes());
        out.extend_from_slice(&snapshot.provider_ref_digest);
        out.extend_from_slice(&(format.len() as u16).to_be_bytes());
        out.extend_from_slice(format);
        out.extend_from_slice(&snapshot.format_version.to_be_bytes());
    }
    out.extend_from_slice(&plan.scope_digest);
    Ok(out)
}

/// Discuss the canonical bytes of the plan.
pub fn decode_plan(bytes: &[u8]) -> Result<MutationPlan, PlanError> {
    let mut cursor = PlanCursor { data: bytes, at: 0 };
    let transition = Transition::from_wire(cursor.u32()? as i32)?;
    let old_id = cursor.arr16()?;
    let old_auth = cursor.arr16()?;
    let old_generation = cursor.u64()?;
    let old_target = if old_id.iter().all(|byte| *byte == 0) {
        // The missing target takes exactly 40 zero bytes. You can't normalize the damaged
        // record to the missing value.
        if old_auth.iter().any(|byte| *byte != 0) || old_generation != 0 {
            return Err(PlanError::OldTargetShape);
        }
        None
    } else {
        Some(EnrollmentRef {
            id: EnrollmentId::from_bytes(old_id).map_err(|_| PlanError::OldTargetShape)?,
            authenticator: AuthenticatorId::from_bytes(old_auth)
                .map_err(|_| PlanError::OldTargetShape)?,
            generation: EnrollmentGeneration::new(old_generation)
                .map_err(|_| PlanError::GenerationShape)?,
        })
    };
    let id = cursor.arr16()?;
    let new_enrollment = if id.iter().all(|byte| *byte == 0) {
        None
    } else {
        Some(EnrollmentId::from_bytes(id).map_err(|_| PlanError::TransitionShape)?)
    };
    let new_generation = cursor.u64()?;
    let count = cursor.u32()? as usize;
    if count > SNAPSHOTS_MAX {
        return Err(PlanError::TooMany { actual: count });
    }
    let mut pre_bindings = Vec::with_capacity(count);
    for _ in 0..count {
        let enrollment_id =
            EnrollmentId::from_bytes(cursor.arr16()?).map_err(|_| PlanError::OutOfScope)?;
        let authenticator =
            AuthenticatorId::from_bytes(cursor.arr16()?).map_err(|_| PlanError::OutOfScope)?;
        let generation =
            EnrollmentGeneration::new(cursor.u64()?).map_err(|_| PlanError::GenerationShape)?;
        let authority = carosign_core::ids::AuthorityId::from_bytes(cursor.arr16()?)
            .map_err(|_| PlanError::OutOfScope)?;
        let subject_id =
            SubjectId::from_bytes(cursor.arr16()?).map_err(|_| PlanError::OutOfScope)?;
        let kind = StoredBindingKind::from_wire(cursor.u32()? as i32)?;
        let mut digest = [0u8; 32];
        digest.copy_from_slice(cursor.take(32)?);
        let format_len = u16::from_be_bytes([cursor.take(1)?[0], cursor.take(1)?[0]]) as usize;
        let format_bytes = cursor.take(format_len)?;
        let format_id = core::str::from_utf8(format_bytes)
            .map_err(|_| PlanError::BadFormat)?
            .to_string();
        let format_version = cursor.u32()?;
        pre_bindings.push(BindingSnapshot {
            enrollment: EnrollmentRef {
                id: enrollment_id,
                authenticator,
                generation,
            },
            subject: SubjectRef {
                authority,
                subject: subject_id,
            },
            kind,
            provider_ref_digest: digest,
            format_id,
            format_version,
        });
    }
    let mut scope_digest = [0u8; 32];
    scope_digest.copy_from_slice(cursor.take(32)?);
    if cursor.at != bytes.len() {
        return Err(PlanError::DigestMismatch);
    }
    Ok(MutationPlan {
        transition,
        old_target,
        new_enrollment,
        new_generation,
        pre_bindings,
        requested_label: None,
        scope_digest,
    })
}
