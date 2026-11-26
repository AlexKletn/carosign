use crate::generated::carosign::broker::v1 as broker;
use crate::generated::carosign::common::v1 as common;
use crate::limits;

use super::{WireError, bounded, bounded_str, check_version, optional_id, require_id};

/// Checking the mediator's frame.
pub fn check_broker_frame(frame: &broker::BrokerFrame) -> Result<(), WireError> {
    use broker::broker_frame::Payload as P;

    check_version(frame.major, frame.minor)?;
    if frame.frame_seq == 0 {
        return Err(WireError::BadFrameSequence {
            expected: 1,
            actual: 0,
        });
    }
    match &frame.payload {
        Some(P::Call(call)) => {
            require_id(&frame.request_id, "request_id")?;
            require_id(&call.operation_ref, "operation_ref")?;
            optional_id(&call.operation_id, "operation_id")?;
            optional_id(&call.mutation_id, "mutation_id")?;
            optional_id(&call.command_id, "command_id")?;
            bounded(&call.payload, "call.payload", limits::BROKER_PAYLOAD_MAX)?;
            if let Some(body) = &call.external_body {
                check_external_body_metadata(frame.minor, body)?;
            }
        }
        Some(P::CallResult(result)) => {
            require_id(&frame.request_id, "request_id")?;
            bounded(
                &result.payload,
                "call_result.payload",
                limits::BROKER_PAYLOAD_MAX,
            )?;
            if let Some(body) = &result.external_body {
                check_external_body_metadata(frame.minor, body)?;
            }
            if let Some(error) = &result.error {
                check_broker_error(error)?;
            }
        }
        Some(P::Ready(ready)) => {
            if !frame.request_id.is_empty() {
                return Err(WireError::UnexpectedPayload {
                    what: "request_id в ServiceReady",
                });
            }
            require_id(&ready.resource_id, "resource_id")?;
        }
        Some(P::Fault(fault)) => {
            let error = fault.error.as_ref().ok_or(WireError::UnexpectedPayload {
                what: "ошибка посредника без описания",
            })?;
            check_broker_error(error)?;
        }
        None => {
            return Err(WireError::UnexpectedPayload {
                what: "кадр посредника без сообщения",
            });
        }
    }
    Ok(())
}

fn check_broker_error(error: &common::Error) -> Result<(), WireError> {
    if error.code == 0 {
        return Err(WireError::UnspecifiedEnum {
            field: "error.code",
        });
    }
    if common::ErrorCode::from_i32(error.code).is_none() {
        return Err(WireError::UnknownEnum {
            field: "error.code",
            value: error.code,
        });
    }
    bounded_str(
        &error.safe_message,
        "error.safe_message",
        limits::SAFE_MESSAGE_MAX,
    )
}

// Form verification does not confirm the contents of the file, its seals or the authority of
// the handler: these checks are performed by the owner of the accepted descriptor.
fn check_external_body_metadata(minor: u32, body: &broker::ExternalBody) -> Result<(), WireError> {
    if minor < 1 {
        return Err(WireError::UnsupportedVersion {
            major: limits::MAJOR,
            minor,
        });
    }
    if body.offset != 0
        || body.length == 0
        || body.length > 8 * 1024 * 1024
        || body.file_size != body.length
        || body.sha256.len() != 32
    {
        return Err(WireError::UnexpectedPayload {
            what: "неверные метаданные внешнего тела",
        });
    }
    Ok(())
}
