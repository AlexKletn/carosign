//! Events of the wire → observation of the engine.
//!
//! ⚠️ `Matched` It is an affirmation, not a success: here it becomes an observation, and the
//! decision is made by the engine after checking the proof.

use carosign_engine::DomainInput;
#[cfg(test)]
use carosign_protocol::carosign::common::v1 as common;
#[cfg(test)]
use carosign_protocol::carosign::provider::v1 as provider;

use crate::codec::RequestError;

/// Display of the event.
pub fn to_domain(
    event: carosign_provider_sdk::events::Event,
) -> Result<DomainInput, RequestError> {
    use carosign_provider_sdk::events::Event;

    match event {
        Event::Accepted { .. }
        | Event::EnrollmentChallenge { .. }
        | Event::CancelReceipt { .. }
        | Event::EnrollmentTransactionResult(_)
        | Event::Quiesced { .. }
        | Event::Described { .. } => {
            // These messages require a command/session context or a separate resource
            // assessment and are not the course of an operation.
            Err(RequestError::Wire(
                carosign_protocol::validation::WireError::UnexpectedPayload {
                    what: "событие требует корреляции с командой или сеансом",
                },
            ))
        }
        Event::Progress { .. } => {
            // As long as the engine input does not tolerate view, interest, and anchoring,
            // turning an event into an empty Progress would lose those fields.
            Err(RequestError::Wire(
                carosign_protocol::validation::WireError::UnexpectedPayload {
                    what: "ход требует проверки сеансом и сохранения полей",
                },
            ))
        }
        Event::Verification { .. } => {
            // Even the negative response must be from the current operation, the worker,
            // the era and the command that is received, and this data and the state of the
            // session are not enough for such verification.
            Err(RequestError::Wire(
                carosign_protocol::validation::WireError::UnexpectedPayload {
                    what: "исход проверки требует корреляции с командой и сеансом",
                },
            ))
        }
        Event::Mutation { .. } => Err(RequestError::Wire(
            carosign_protocol::validation::WireError::UnexpectedPayload {
                what: "исход изменения требует проверки сеансом и владельцем операции",
            },
        )),
        Event::QueryState { .. } | Event::Fault { .. } => Err(RequestError::Wire(
            carosign_protocol::validation::WireError::UnexpectedPayload {
                what: "событие требует корреляции с командой и сеансом",
            },
        )),
    }
}

#[path = "provider.tests.rs"]
#[cfg(test)]
mod tests;
