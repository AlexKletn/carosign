use super::super::ProviderFailure;

use super::super::mutate_plan;
use carosign_core::ids::{CommandId, MutationId};

use super::*;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::ids::{AuthenticatorId, EnrollmentId};
use carosign_runtime::{BindingKind, EnrollmentBinding};

struct Dummy;

impl ProviderChannel for Dummy {
    fn send(&self, _frame: provider::ProviderFrame) -> Result<(), ProviderFailure> {
        Ok(())
    }
    fn cancel(&self, _operation: OperationId) -> bool {
        true
    }
}

fn binding() -> EnrollmentBinding {
    EnrollmentBinding {
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([7u8; 16]).expect("валид"),
            authenticator: AuthenticatorId::from_bytes([9u8; 16]).expect("валид"),
            generation: EnrollmentGeneration::FIRST,
        },
        kind: BindingKind::ProviderOpaque,
        provider_ref: vec![1, 2, 3],
        format_id: Some("goodix-asp".into()),
        format_version: 1,
    }
}

#[test]
fn removal_carries_the_binding() {
    // ⚠️ D-07: Deletion carries a bundle, not just a link.
    let binding = binding();
    let plan = mutate_plan(
        MutationId::from_bytes([3u8; 16]).expect("валид"),
        CommandId::from_bytes([4u8; 16]).expect("валид"),
        EnrollmentAuthorization::None,
        MutationChange::Removal {
            enrollment: binding.enrollment,
            binding: binding.clone(),
        },
    );
    let frame = ProviderBridge::<Dummy>::frame(
        [1u8; 16],
        1,
        OperationId::from_bytes([2u8; 16]).expect("валид"),
        &plan,
    );
    match frame.payload {
        Some(Payload::StartRemoval(start)) => {
            let carried = start.enrollment_binding.expect("связка есть");
            assert_eq!(carried.enrollment.expect("ссылка").generation, 1);
            assert_eq!(
                carried.kind,
                provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32
            );
            assert_eq!(carried.provider_opaque_ref, vec![1, 2, 3]);
        }
        other => panic!("ожидалось удаление, а пришло {other:?}"),
    }
}
