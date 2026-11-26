use super::*;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::ids::{AuthenticatorId, EnrollmentId};

fn reference(byte: u8) -> EnrollmentRef {
    EnrollmentRef {
        id: EnrollmentId::from_bytes([byte; 16]).expect("валид"),
        authenticator: AuthenticatorId::from_bytes([3u8; 16]).expect("валид"),
        generation: EnrollmentGeneration::FIRST,
    }
}

#[test]
fn create_maps_to_create() {
    let change = to_mutation_change(Projection {
        change: ProjectionChange::Create {
            planned_new: reference(1),
        },
        scope_size: 0,
    });
    assert!(matches!(change, MutationChange::Create { .. }));
}

#[test]
fn replace_maps_to_replace_with_binding() {
    let change = to_mutation_change(Projection {
        change: ProjectionChange::Replace {
            planned_new: reference(2),
            old: carosign_persistence::projection::EnrollmentBinding {
                enrollment: reference(1),
                kind: carosign_persistence::mutation_plan::StoredBindingKind::ProviderOpaque,
                provider_ref: vec![9, 9],
                format_id: Some("goodix-asp".into()),
                format_version: 1,
            },
        },
        scope_size: 1,
    });
    match change {
        MutationChange::Replace { planned_new, old } => {
            assert_eq!(planned_new.id, reference(2).id);
            assert_eq!(old.enrollment.id, reference(1).id);
            assert_eq!(old.provider_ref, vec![9, 9]);
        }
        other => panic!("ожидалась замена, а пришло {other:?}"),
    }
}
