//! Evidence and authority: what is accepted and what is not.
//!
//! ⚠️ The main property of the platform is checked: the software match **never ** becomes a
//! hardware guarantee; the inaccessible checker is a failure, not indulgence; the material is
//! one-time.

use carosign_core::assurance::AssuranceClass;
use carosign_core::ids::{AuthorityId, BootId, SubjectId};
use carosign_core::operation::context::{BootInstant, VerificationPurpose};
use carosign_engine::VerifiedState;
use carosign_evidence::assessment::Assessment;
use carosign_evidence::material::EvidenceError;
use carosign_evidence::policy::{FactorDecision, TrustProfile, decide};
use carosign_evidence::recipient::{ChallengeRef, RecipientRef};
use carosign_evidence::requirements::EvidenceRequirements;
use carosign_evidence::routing::{Held, RoutingError};
use carosign_evidence::verifier::{EvidenceContext, Observed, ProtectedAnswer, Verdict, verify};

fn evidence_fixture(
    id: [u8; 16],
    operation: [u8; 16],
) -> Result<carosign_evidence::material::EvidenceMaterialRef, ()> {
    use carosign_core::operation::context::{BootDeadline, BootInstant};
    let boot = carosign_core::ids::BootId::from_bytes([42; 16]).unwrap();
    let slot = carosign_evidence::store::EvidenceSlot::new(
        carosign_core::ids::EvidenceId::from_bytes(id).map_err(|_| ())?,
        carosign_core::ids::OperationId::from_bytes(operation).map_err(|_| ())?,
        BootDeadline::new(boot, 20),
    );
    slot.hold(zeroize::Zeroizing::new(vec![1]), BootInstant::new(boot, 10))
        .map_err(|_| ())
}

fn boot() -> BootId {
    BootId::from_bytes([1u8; 16]).expect("валид")
}

#[test]
fn diagnostics_do_not_print_evidence_bindings_or_challenge() {
    let context_debug = format!("{:?}", context());
    for field in [
        "operation",
        "subject",
        "enrollment",
        "recipient",
        "challenge",
    ] {
        assert!(!context_debug.contains(field), "{field}: {context_debug}");
    }
    let observed_debug = format!("{:?}", observed());
    assert!(!observed_debug.contains("recipient"));
    assert!(!observed_debug.contains("challenge"));
    assert!(observed_debug.contains("protected"));
}

fn state() -> VerifiedState {
    VerifiedState {
        enrollment: carosign_core::ids::EnrollmentId::from_bytes([3u8; 16]).expect("валид"),
        generation: carosign_core::enrollment::EnrollmentGeneration::FIRST,
        provenance: carosign_core::ids::EvidenceId::from_bytes([4u8; 16]).expect("валид"),
    }
}

fn context() -> EvidenceContext {
    EvidenceContext {
        operation: [5u8; 16],
        subject: {
            let mut subject = [0u8; 32];
            subject[..16].copy_from_slice(
                AuthorityId::from_bytes([6u8; 16])
                    .expect("валид")
                    .as_bytes(),
            );
            subject[16..]
                .copy_from_slice(SubjectId::from_bytes([7u8; 16]).expect("валид").as_bytes());
            subject
        },
        enrollment: state(),
        purpose: VerificationPurpose::SessionLogin,
        recipient: RecipientRef::new([8u8; 16]).expect("получатель"),
        challenge: ChallengeRef::accepted_by_recipient([9u8; 16]).expect("вызов"),
        deadline: BootInstant::new(boot(), 0).deadline_after(1_000),
    }
}

fn observed() -> Observed {
    Observed {
        recipient: [8u8; 16],
        challenge: [9u8; 16],
        hardware_path: false,
        protected: ProtectedAnswer::Verified,
    }
}

#[test]
fn untrusted_software_observation_is_not_accepted() {
    let material = evidence_fixture([1u8; 16], [5u8; 16]).expect("материал");
    let verdict = verify(
        &material,
        &context(),
        &EvidenceRequirements::local_login(),
        BootInstant::new(boot(), 10),
        observed(),
    );
    assert!(matches!(
        verdict,
        Verdict::Rejected(EvidenceError::UnverifiedProvenance)
    ));
}

#[test]
fn software_match_never_becomes_hardware() {
    let material = evidence_fixture([1u8; 16], [5u8; 16]).expect("материал");
    let mut key_context = context();
    key_context.purpose = VerificationPurpose::KeyOperation;
    let verdict = verify(
        &material,
        &key_context,
        &EvidenceRequirements::key_operation(),
        BootInstant::new(boot(), 10),
        observed(),
    );
    // ⚠️ For the operation with the key, the hardware path is mandatory - failure.
    assert!(matches!(
        verdict,
        Verdict::Rejected(EvidenceError::IncompatibleAssurance)
    ));
}

#[test]
fn claimed_hardware_path_does_not_prove_key_operation() {
    let material = evidence_fixture([1u8; 16], [5u8; 16]).expect("материал");
    let mut context = context();
    context.purpose = VerificationPurpose::KeyOperation;
    let mut observed = observed();
    observed.hardware_path = true;
    let verdict = verify(
        &material,
        &context,
        &EvidenceRequirements::key_operation(),
        BootInstant::new(boot(), 10),
        observed,
    );
    assert!(matches!(
        verdict,
        Verdict::Rejected(EvidenceError::UnverifiedProvenance)
    ));
}

#[test]
fn unavailable_verifier_is_refusal_not_downgrade() {
    let material = evidence_fixture([1u8; 16], [5u8; 16]).expect("материал");
    let mut observed = observed();
    observed.protected = ProtectedAnswer::Unavailable;
    let verdict = verify(
        &material,
        &context(),
        &EvidenceRequirements::local_login(),
        BootInstant::new(boot(), 10),
        observed,
    );
    assert!(matches!(
        verdict,
        Verdict::Rejected(EvidenceError::VerifierUnavailable)
    ));
}

#[test]
fn expired_material_never_reaches_the_module() {
    let material = evidence_fixture([1u8; 16], [5u8; 16]).expect("материал");
    let verdict = verify(
        &material,
        &context(),
        &EvidenceRequirements::local_login(),
        BootInstant::new(boot(), 5_000),
        observed(),
    );
    assert!(matches!(verdict, Verdict::Rejected(EvidenceError::Expired)));
}

#[test]
fn wrong_recipient_and_challenge_are_refused() {
    let material = evidence_fixture([1u8; 16], [5u8; 16]).expect("материал");
    let mut wrong_recipient = observed();
    wrong_recipient.recipient = [1u8; 16];
    assert!(matches!(
        verify(
            &material,
            &context(),
            &EvidenceRequirements::local_login(),
            BootInstant::new(boot(), 1),
            wrong_recipient
        ),
        Verdict::Rejected(EvidenceError::WrongRecipient)
    ));

    let mut wrong_challenge = observed();
    wrong_challenge.challenge = [2u8; 16];
    assert!(matches!(
        verify(
            &material,
            &context(),
            &EvidenceRequirements::local_login(),
            BootInstant::new(boot(), 1),
            wrong_challenge
        ),
        Verdict::Rejected(EvidenceError::ChallengeMismatch)
    ));
}

#[test]
fn material_from_another_operation_is_refused() {
    let material = evidence_fixture([1u8; 16], [99u8; 16]).expect("материал");
    assert!(matches!(
        verify(
            &material,
            &context(),
            &EvidenceRequirements::local_login(),
            BootInstant::new(boot(), 1),
            observed()
        ),
        Verdict::Rejected(EvidenceError::MalformedEvidence)
    ));
}

#[test]
fn requirements_must_match_purpose() {
    // The goal is the key, and the hardware warranty is not required: this is a different
    // model of trust, and it is forbidden to accept it silently.
    let wrong = EvidenceRequirements {
        purpose: VerificationPurpose::KeyOperation,
        hardware_required: false,
        fresh_challenge: true,
    };
    assert!(wrong.validate().is_err());
}

#[test]
fn requirements_cannot_be_used_for_a_different_operation_purpose() {
    let material = evidence_fixture([1; 16], [5; 16]).unwrap();
    let mut key_context = context();
    key_context.purpose = VerificationPurpose::KeyOperation;
    assert!(matches!(
        verify(
            &material,
            &key_context,
            &EvidenceRequirements::local_login(),
            BootInstant::new(boot(), 1),
            observed()
        ),
        Verdict::Rejected(EvidenceError::IncompatibleAssurance)
    ));
    assert!(matches!(
        verify(
            &material,
            &context(),
            &EvidenceRequirements::key_operation(),
            BootInstant::new(boot(), 1),
            observed()
        ),
        Verdict::Rejected(EvidenceError::IncompatibleAssurance)
    ));
}

#[test]
fn policy_refuses_software_for_key_operations() {
    assert_eq!(
        decide(
            VerificationPurpose::KeyOperation,
            TrustProfile::LocalLogin,
            AssuranceClass::SoftwareAsserted
        ),
        FactorDecision::Refused
    );
    assert_eq!(
        decide(
            VerificationPurpose::SessionLogin,
            TrustProfile::LocalLogin,
            AssuranceClass::SoftwareAsserted
        ),
        FactorDecision::Accepted
    );
    // ⚠️ Unknown origin is not accepted under any profile.
    assert_eq!(
        decide(
            VerificationPurpose::SessionLogin,
            TrustProfile::LocalLogin,
            AssuranceClass::Unverified
        ),
        FactorDecision::Refused
    );
}

#[test]
fn policy_never_reuses_a_profile_for_another_purpose() {
    for class in [
        AssuranceClass::Unverified,
        AssuranceClass::SoftwareAsserted,
        AssuranceClass::HardwareVerified,
    ] {
        assert_eq!(
            decide(
                VerificationPurpose::KeyOperation,
                TrustProfile::LocalLogin,
                class
            ),
            FactorDecision::Refused
        );
        assert_eq!(
            decide(
                VerificationPurpose::SessionLogin,
                TrustProfile::KeyOperation,
                class
            ),
            FactorDecision::Refused
        );
    }
    assert_eq!(
        decide(
            VerificationPurpose::KeyOperation,
            TrustProfile::KeyOperation,
            AssuranceClass::HardwareVerified,
        ),
        FactorDecision::Accepted
    );
}

#[test]
fn material_is_consumed_once() {
    let reference = evidence_fixture([1u8; 16], [5u8; 16]).expect("материал");
    let mut held = Held::hold(reference);
    assert!(held.take([5u8; 16]).is_ok());
    assert_eq!(held.take([5u8; 16]), Err(RoutingError::AlreadyConsumed));
}

#[test]
fn assessment_does_not_upgrade_by_wrapping() {
    let software = Assessment::software();
    let domain = software.into_domain(
        carosign_core::ids::EvidenceId::from_bytes([1u8; 16]).expect("валид"),
        carosign_core::ids::OperationId::from_bytes([2u8; 16]).expect("валид"),
        carosign_core::ids::EnrollmentId::from_bytes([3u8; 16]).expect("валид"),
        carosign_core::ids::AuthenticatorId::from_bytes([4u8; 16]).expect("валид"),
    );
    assert!(!domain.satisfies_key_operation());
}

#[test]
fn provider_assertion_is_never_hardware() {
    // ⚠️ Document 29, D-06: A match without a token is a provider’s approval, not a hardware
    // warranty.
    let assertion = Assessment::provider_assertion();
    assert_eq!(assertion.class, AssuranceClass::SoftwareAsserted);
    assert!(
        !assertion
            .into_domain(
                carosign_core::ids::EvidenceId::from_bytes([1u8; 16]).expect("валид"),
                carosign_core::ids::OperationId::from_bytes([2u8; 16]).expect("валид"),
                carosign_core::ids::EnrollmentId::from_bytes([3u8; 16]).expect("валид"),
                carosign_core::ids::AuthenticatorId::from_bytes([4u8; 16]).expect("валид"),
            )
            .satisfies_key_operation()
    );
}

#[test]
fn hardware_policy_refuses_provider_assertion() {
    assert_eq!(
        decide(
            VerificationPurpose::KeyOperation,
            TrustProfile::KeyOperation,
            Assessment::provider_assertion().class
        ),
        FactorDecision::Refused
    );
}
