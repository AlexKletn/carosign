use super::*;
use carosign_protocol::carosign::provider::v1::provider_frame::Payload;

fn frame(payload: Payload) -> provider::ProviderFrame {
    provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        worker_instance_id: vec![1; 16],
        frame_seq: 1,
        command_id: Vec::new(),
        operation_id: Vec::new(),
        device_epoch: 1,
        uid: None,
        payload: Some(payload),
    }
}

#[test]
fn bind_is_refused_before_negotiation() {
    let bind = frame(Payload::BindAuthenticator(
        provider::BindAuthenticator::default(),
    ));
    assert!(!accepts(Phase::Starting, &bind));
    assert!(accepts(Phase::Binding, &bind));
}

#[test]
fn invalid_phase_transition_faults_instead_of_skipping() {
    assert_eq!(
        transition(Phase::Starting, HandshakeStep::SendInitialized),
        Phase::Faulted
    );
    assert_eq!(
        transition(Phase::Starting, HandshakeStep::SendReady),
        Phase::Faulted
    );
    assert_eq!(
        transition(Phase::Starting, HandshakeStep::SendHello),
        Phase::Negotiating
    );
    assert_eq!(
        transition(Phase::Negotiating, HandshakeStep::SendInitialized),
        Phase::Binding
    );
    assert_eq!(
        transition(Phase::Binding, HandshakeStep::SendReady),
        Phase::Ready
    );
    assert_eq!(
        transition(Phase::Faulted, HandshakeStep::SendHello),
        Phase::Faulted
    );
}

#[test]
fn prebind_declaration_requires_explicit_negotiation() {
    use carosign_protocol::carosign::provider::v1::{PrebindDescribe, provider_frame::Payload};
    let frame = provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        payload: Some(Payload::PrebindDescribe(PrebindDescribe::default())),
        ..Default::default()
    };
    assert!(!accepts(Phase::Binding, &frame));
    assert!(!accepts_with_features(Phase::Binding, &frame, false));
    assert!(accepts_with_features(Phase::Binding, &frame, true));
    assert!(!accepts_with_features(Phase::Ready, &frame, true));
}

#[test]
fn initialize_requires_supported_matching_minor() {
    let mut init = frame(Payload::Initialize(provider::Initialize {
        selected_minor: carosign_protocol::limits::MINOR_BASELINE,
        ..Default::default()
    }));
    assert!(!accepts(Phase::Starting, &init));
    assert!(accepts(Phase::Negotiating, &init));
    init.minor = carosign_protocol::limits::MINOR_BASELINE - 1;
    assert!(!accepts(Phase::Negotiating, &init));
    if let Some(Payload::Initialize(value)) = &mut init.payload {
        value.selected_minor = init.minor;
    }
    assert!(!accepts(Phase::Negotiating, &init));
}

#[test]
fn commands_are_refused_until_ready() {
    let mut start = frame(Payload::StartVerification(
        provider::StartVerification::default(),
    ));
    assert!(!accepts(Phase::Binding, &start));
    assert!(accepts(Phase::Ready, &start));
    start.minor = carosign_protocol::limits::MINOR_BASELINE - 1;
    assert!(!accepts(Phase::Ready, &start));
    start.minor = carosign_protocol::limits::MINOR_MAX + 1;
    assert!(!accepts(Phase::Ready, &start));
}

#[test]
fn enrollment_continuation_only_accepted_when_ready() {
    let value = frame(Payload::ContinueEnrollment(
        provider::ContinueEnrollment::default(),
    ));
    assert!(!accepts(Phase::Binding, &value));
    assert!(accepts(Phase::Ready, &value));
    assert!(!accepts(Phase::Draining, &value));
    assert!(!accepts(Phase::Faulted, &value));
}

#[test]
fn binding_and_control_messages_require_supported_minor() {
    for (phase, payload) in [
        (
            Phase::Binding,
            Payload::BindAuthenticator(provider::BindAuthenticator::default()),
        ),
        (
            Phase::Ready,
            Payload::Describe(provider::Describe::default()),
        ),
        (
            Phase::Ready,
            Payload::RequestCancel(provider::RequestCancel::default()),
        ),
        (
            Phase::Draining,
            Payload::Shutdown(provider::Shutdown::default()),
        ),
    ] {
        let mut value = frame(payload);
        assert!(accepts(phase, &value));
        value.minor = carosign_protocol::limits::MINOR_BASELINE - 1;
        assert!(!accepts(phase, &value));
        value.minor = carosign_protocol::limits::MINOR_MAX + 1;
        assert!(!accepts(phase, &value));
    }
}

#[test]
fn cancel_is_accepted_while_working() {
    let cancel = frame(Payload::RequestCancel(provider::RequestCancel::default()));
    assert!(accepts(Phase::Ready, &cancel));
    assert!(accepts(Phase::Draining, &cancel));
    assert!(!accepts(Phase::Binding, &cancel));
}

#[test]
fn faulted_accepts_nothing() {
    let quiesce = frame(Payload::Quiesce(provider::Quiesce::default()));
    assert!(!accepts(Phase::Faulted, &quiesce));
}
