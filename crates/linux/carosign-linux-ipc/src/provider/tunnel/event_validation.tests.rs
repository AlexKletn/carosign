use super::*;
use carosign_protocol::carosign::{
    broker_relay::v1::{Event, Frame, frame::Body},
    common::v1::{
        AuthenticatorRef, PresentationAttackDetection,
        PresentationAttackDetectionSupport as Support, PresentationAttackType as Attack,
    },
    provider::v1::{AuthenticatorReady, ProviderFrame, provider_frame::Payload},
};

#[test]
fn bind_reply_accepts_only_consistent_assurance_declarations() {
    let frame = |support, covered_attacks| Frame {
        body: Some(Body::Event(Event {
            frame: Some(ProviderFrame {
                payload: Some(Payload::AuthenticatorReady(AuthenticatorReady {
                    authenticator: Some(AuthenticatorRef {
                        authenticator_id: vec![4; 16],
                        device_epoch: 1,
                    }),
                    capabilities: Some(CapabilityDescriptor {
                        assurance: Some(DeclaredAssurance {
                            presentation_attack_detection: Some(PresentationAttackDetection {
                                support: support as i32,
                                covered_attacks,
                            }),
                        }),
                        ..Default::default()
                    }),
                })),
                ..Default::default()
            }),
        })),
        ..Default::default()
    };
    let photo = Attack::PrintedPhoto as i32;
    for (support, coverage, accepted) in [
        (Support::Declared, vec![photo], true),
        (Support::Declared, vec![], true),
        (Support::Unknown, vec![], true),
        (Support::Unsupported, vec![photo], false),
        (Support::Declared, vec![photo, photo], false),
        (Support::Declared, vec![i32::MAX], false),
    ] {
        assert_eq!(
            strict_event(&frame(support, coverage).encode_to_vec()).is_ok(),
            accepted,
        );
    }
}
