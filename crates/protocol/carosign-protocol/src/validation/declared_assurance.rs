use crate::carosign::common::v1::{
    CapabilityDescriptor, PresentationAttackDetectionSupport as Support,
    PresentationAttackType as Attack,
};

use super::WireError;

pub fn check_declared_assurance(descriptor: &CapabilityDescriptor) -> Result<(), WireError> {
    let Some(detection) = descriptor
        .assurance
        .as_ref()
        .and_then(|assurance| assurance.presentation_attack_detection.as_ref())
    else {
        return Ok(());
    };
    let support = Support::from_i32(detection.support).ok_or(WireError::UnknownEnum {
        field: "assurance.presentation_attack_detection.support",
        value: detection.support,
    })?;
    if support != Support::Declared && !detection.covered_attacks.is_empty() {
        return Err(WireError::UnexpectedPayload {
            what: "покрытие без заявленного обнаружения атак предъявления",
        });
    }
    let mut seen = 0u8;
    for &value in &detection.covered_attacks {
        let bit = match Attack::from_i32(value) {
            Some(Attack::PrintedPhoto) => 1,
            Some(Attack::ScreenReplay) => 2,
            Some(Attack::Unspecified) => {
                return Err(WireError::UnspecifiedEnum {
                    field: "assurance.presentation_attack_detection.covered_attacks",
                });
            }
            None => {
                return Err(WireError::UnknownEnum {
                    field: "assurance.presentation_attack_detection.covered_attacks",
                    value,
                });
            }
        };
        if seen & bit != 0 {
            return Err(WireError::UnexpectedPayload {
                what: "повтор вида атаки предъявления",
            });
        }
        seen |= bit;
    }
    Ok(())
}
