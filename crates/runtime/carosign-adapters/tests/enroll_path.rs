//! Throughway registration: plan → registry → projection → frame provider.
//!
//! ⚠️ Checks what previously did not converge in one test: a plan from the vault, checking
//! the old bundle and a frame of the wire with the `planned_enrollment`.

use carosign_adapters::provider::{ProviderBridge, ProviderChannel, ProviderFailure, mutate_plan};
use carosign_adapters::to_mutation_change;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, CommandId, EnrollmentId, MutationId, OperationId, SubjectId,
};
use carosign_persistence::enrollment_codec::EnrollmentStatus;
use carosign_persistence::mutation_plan::{
    BindingSnapshot, MutationPlan, Scope, StoredBindingKind, Transition, sha256,
};
use carosign_persistence::projection::{RegistryEntry, build};
use carosign_protocol::carosign::provider::v1 as provider;
use carosign_protocol::carosign::provider::v1::provider_frame::Payload;
use carosign_runtime::ports::EnrollmentAuthorization;

struct Sink;

impl ProviderChannel for Sink {
    fn send(&self, _frame: provider::ProviderFrame) -> Result<(), ProviderFailure> {
        Ok(())
    }
    fn cancel(&self, _operation: OperationId) -> bool {
        true
    }
}

fn subject() -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
    }
}

fn authenticator() -> AuthenticatorId {
    AuthenticatorId::from_bytes([3u8; 16]).expect("валид")
}

fn reference(byte: u8) -> EnrollmentRef {
    EnrollmentRef {
        id: EnrollmentId::from_bytes([byte; 16]).expect("валид"),
        authenticator: authenticator(),
        generation: EnrollmentGeneration::FIRST,
    }
}

fn entry(byte: u8, reference_bytes: &[u8]) -> RegistryEntry {
    RegistryEntry {
        enrollment: reference(byte),
        subject: subject(),
        status: EnrollmentStatus::Active,
        kind: StoredBindingKind::ProviderOpaque,
        provider_ref: reference_bytes.to_vec(),
        format_id: "goodix-asp".into(),
        format_version: 1,
    }
}

fn snapshot(entry: &RegistryEntry) -> BindingSnapshot {
    BindingSnapshot {
        enrollment: entry.enrollment,
        subject: entry.subject,
        kind: entry.kind,
        provider_ref_digest: sha256(&entry.provider_ref),
        format_id: entry.format_id.clone(),
        format_version: entry.format_version,
    }
}

#[test]
fn replace_goes_from_plan_to_wire_frame() {
    let old = entry(1, b"old");
    let other = entry(2, b"other");
    let plan = MutationPlan::build(
        Transition::EnrollReplace,
        Some(old.enrollment),
        Some(reference(9).id),
        1,
        vec![snapshot(&old), snapshot(&other)],
        Scope {
            subject: subject(),
            authenticator: authenticator(),
        },
    )
    .expect("план");

    let projection = build(
        &plan,
        Scope {
            subject: subject(),
            authenticator: authenticator(),
        },
        &[old.clone(), other],
    )
    .expect("проекция");
    let change = to_mutation_change(projection);
    let plan_command = mutate_plan(
        MutationId::from_bytes([4u8; 16]).expect("валид"),
        CommandId::from_bytes([5u8; 16]).expect("валид"),
        EnrollmentAuthorization::None,
        change,
    );
    let frame = ProviderBridge::<Sink>::frame(
        [1u8; 16],
        1,
        OperationId::from_bytes([6u8; 16]).expect("валид"),
        &plan_command,
    );
    match frame.payload {
        Some(Payload::StartEnrollment(start)) => {
            // ⚠️ D-34: new - in `planned_enrollment`The old one is in both fields.
            let planned = start.planned_enrollment.expect("новое назначено");
            assert_eq!(planned.enrollment_id, reference(9).id.as_bytes().to_vec());
            let replace = start.replace.expect("замена");
            assert_eq!(replace.enrollment_id, old.enrollment.id.as_bytes().to_vec());
            assert!(start.replace_binding.is_some());
        }
        other => panic!("ожидалась регистрация, а пришло {other:?}"),
    }
}

#[test]
fn create_frame_has_no_replace_fields() {
    let plan = MutationPlan::build(
        Transition::EnrollNew,
        None,
        Some(reference(9).id),
        1,
        Vec::new(),
        Scope {
            subject: subject(),
            authenticator: authenticator(),
        },
    )
    .expect("план");
    let projection = build(
        &plan,
        Scope {
            subject: subject(),
            authenticator: authenticator(),
        },
        &[],
    )
    .expect("проекция");
    let plan_command = mutate_plan(
        MutationId::from_bytes([4u8; 16]).expect("валид"),
        CommandId::from_bytes([5u8; 16]).expect("валид"),
        EnrollmentAuthorization::None,
        to_mutation_change(projection),
    );
    let frame = ProviderBridge::<Sink>::frame(
        [1u8; 16],
        1,
        OperationId::from_bytes([6u8; 16]).expect("валид"),
        &plan_command,
    );
    match frame.payload {
        Some(Payload::StartEnrollment(start)) => {
            let planned = start.planned_enrollment.expect("новое назначено");
            assert_eq!(planned.enrollment_id, reference(9).id.as_bytes().to_vec());
            assert_eq!(
                planned.authenticator_id,
                authenticator().as_bytes().to_vec()
            );
            assert!(start.replace.is_none());
            assert!(start.replace_binding.is_none());
        }
        other => panic!("ожидалась регистрация, а пришло {other:?}"),
    }
}
