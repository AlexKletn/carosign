use prost::encoding::{DecodeContext, WireType, decode_key, decode_varint, skip_field};

use super::TunnelError;
use carosign_protocol::{
    carosign::common::v1::{CapabilityDescriptor, DeclaredAssurance},
    validation::check_declared_assurance,
};
use prost::Message;

// Enclosed fields are checked to prost.As long as only allowed WorkerHello and Initialized,
// CommandAck, ProviderProgress and VerificationOutcomeThe rest of the events are closed until
// their full scheme is checked.
pub(super) fn strict_event(bytes: &[u8]) -> Result<&[u8], TunnelError> {
    fn fields<'a>(
        mut bytes: &'a [u8],
        schema: &[(u32, WireType)],
        repeated: u32,
        nested: u32,
    ) -> Result<Option<&'a [u8]>, TunnelError> {
        let mut seen = 0u128;
        let mut child = None;
        while !bytes.is_empty() {
            let (tag, wire) = decode_key(&mut bytes).map_err(|_| TunnelError::Invalid)?;
            if tag >= 128 || !schema.iter().any(|(t, w)| *t == tag && *w == wire) {
                return Err(TunnelError::Invalid);
            }
            let bit = 1u128 << tag;
            if tag != repeated && seen & bit != 0 {
                return Err(TunnelError::Invalid);
            }
            seen |= bit;
            if tag == nested {
                let len =
                    usize::try_from(decode_varint(&mut bytes).map_err(|_| TunnelError::Invalid)?)
                        .map_err(|_| TunnelError::Invalid)?;
                if len > bytes.len() {
                    return Err(TunnelError::Invalid);
                }
                let (value, rest) = bytes.split_at(len);
                bytes = rest;
                child = Some(value);
            } else {
                skip_field(wire, tag, &mut bytes, DecodeContext::default())
                    .map_err(|_| TunnelError::Invalid)?;
            }
        }
        Ok(child)
    }
    let relay = fields(
        bytes,
        &[
            (1, WireType::Varint),
            (2, WireType::Varint),
            (3, WireType::Varint),
            (4, WireType::LengthDelimited),
            (5, WireType::Varint),
            (12, WireType::LengthDelimited),
        ],
        0,
        12,
    )?
    .ok_or(TunnelError::Invalid)?;
    let provider =
        fields(relay, &[(1, WireType::LengthDelimited)], 0, 1)?.ok_or(TunnelError::Invalid)?;
    let mut payload = None;
    let mut rest = provider;
    let mut seen = 0u128;
    while !rest.is_empty() {
        let (tag, wire) = decode_key(&mut rest).map_err(|_| TunnelError::Invalid)?;
        let expected = match tag {
            1 | 2 | 4 | 7 => WireType::Varint,
            3 | 5 | 6 | 10 | 12 | 14 | 30 | 31 | 32 | 33 | 35 | 36 | 38 | 39 => {
                WireType::LengthDelimited
            }
            _ => return Err(TunnelError::Invalid),
        };
        if wire != expected || seen & (1u128 << tag) != 0 {
            return Err(TunnelError::Invalid);
        }
        seen |= 1u128 << tag;
        if matches!(tag, 10 | 12 | 14 | 30 | 31 | 32 | 33 | 35 | 36 | 38 | 39) {
            if payload.is_some() {
                return Err(TunnelError::Invalid);
            }
            let len = usize::try_from(decode_varint(&mut rest).map_err(|_| TunnelError::Invalid)?)
                .map_err(|_| TunnelError::Invalid)?;
            if len > rest.len() {
                return Err(TunnelError::Invalid);
            }
            let (body, tail) = rest.split_at(len);
            rest = tail;
            payload = Some((tag, body));
        } else {
            skip_field(wire, tag, &mut rest, DecodeContext::default())
                .map_err(|_| TunnelError::Invalid)?;
        }
    }
    match payload.ok_or(TunnelError::Invalid)? {
        (36, body) => {
            fields(
                body,
                &[
                    (1, WireType::LengthDelimited),
                    (2, WireType::LengthDelimited),
                    (3, WireType::LengthDelimited),
                    (4, WireType::Varint),
                ],
                0,
                0,
            )?;
        }
        (38, body) => {
            let description =
                carosign_protocol::carosign::provider::v1::PrebindDescription::decode(body)
                    .map_err(|_| TunnelError::Invalid)?;
            if description.encode_to_vec() != body
                || description
                    .capabilities
                    .as_ref()
                    .is_some_and(|capabilities| check_declared_assurance(capabilities).is_err())
            {
                return Err(TunnelError::Invalid);
            }
        }
        (10, body) => {
            fields(
                body,
                &[
                    (1, WireType::Varint),
                    (2, WireType::Varint),
                    (3, WireType::Varint),
                    (4, WireType::Varint),
                    (4, WireType::LengthDelimited),
                    (5, WireType::LengthDelimited),
                ],
                4,
                0,
            )?;
        }
        (12, body) => {
            fields(body, &[(1, WireType::Varint)], 0, 0)?;
        }
        (14, mut body) => {
            let mut seen = 0u8;
            while !body.is_empty() {
                let (tag, wire) = decode_key(&mut body).map_err(|_| TunnelError::Invalid)?;
                if !(tag == 1 || tag == 2)
                    || wire != WireType::LengthDelimited
                    || seen & (1 << tag) != 0
                {
                    return Err(TunnelError::Invalid);
                }
                seen |= 1 << tag;
                let len =
                    usize::try_from(decode_varint(&mut body).map_err(|_| TunnelError::Invalid)?)
                        .map_err(|_| TunnelError::Invalid)?;
                if len > body.len() {
                    return Err(TunnelError::Invalid);
                }
                let (mut nested, rest) = body.split_at(len);
                body = rest;
                if tag == 1 {
                    fields(
                        nested,
                        &[(1, WireType::LengthDelimited), (2, WireType::Varint)],
                        0,
                        0,
                    )?;
                } else {
                    let capabilities =
                        CapabilityDescriptor::decode(nested).map_err(|_| TunnelError::Invalid)?;
                    check_declared_assurance(&capabilities).map_err(|_| TunnelError::Invalid)?;
                    let mut capability_seen = 0u32;
                    while !nested.is_empty() {
                        let (field, kind) =
                            decode_key(&mut nested).map_err(|_| TunnelError::Invalid)?;
                        let valid = match field {
                            1 | 3 | 5 | 7 | 8 | 10 | 13 | 15 => kind == WireType::Varint,
                            2 | 4 | 11 | 12 => {
                                kind == WireType::Varint || kind == WireType::LengthDelimited
                            }
                            6 | 14 | 16 => kind == WireType::LengthDelimited,
                            _ => false,
                        };
                        if !valid
                            || (!matches!(field, 2 | 4 | 11 | 12)
                                && capability_seen & (1 << field) != 0)
                        {
                            return Err(TunnelError::Invalid);
                        }
                        capability_seen |= 1 << field;
                        if field == 14 {
                            let length = usize::try_from(
                                decode_varint(&mut nested).map_err(|_| TunnelError::Invalid)?,
                            )
                            .map_err(|_| TunnelError::Invalid)?;
                            let (body, rest) = nested
                                .split_at_checked(length)
                                .ok_or(TunnelError::Invalid)?;
                            let assurance = DeclaredAssurance::decode(body)
                                .map_err(|_| TunnelError::Invalid)?;
                            if assurance.encode_to_vec() != body {
                                return Err(TunnelError::Invalid);
                            }
                            nested = rest;
                        } else {
                            skip_field(kind, field, &mut nested, DecodeContext::default())
                                .map_err(|_| TunnelError::Invalid)?;
                        }
                    }
                }
            }
            if seen != 6 {
                return Err(TunnelError::Invalid);
            }
        }
        (30, body) => {
            if let Some(error) = fields(
                body,
                &[(1, WireType::Varint), (2, WireType::LengthDelimited)],
                0,
                2,
            )? {
                fields(
                    error,
                    &[(1, WireType::Varint), (2, WireType::LengthDelimited)],
                    0,
                    0,
                )?;
            }
        }
        (33, mut body) => {
            let mut seen = 0u8;
            while !body.is_empty() {
                let (tag, wire) = decode_key(&mut body).map_err(|_| TunnelError::Invalid)?;
                let expected = match tag {
                    2 => WireType::Varint,
                    1 | 3 | 4 | 5 | 6 => WireType::LengthDelimited,
                    _ => return Err(TunnelError::Invalid),
                };
                if wire != expected || seen & (1 << tag) != 0 {
                    return Err(TunnelError::Invalid);
                }
                seen |= 1 << tag;
                if tag == 3 || tag == 6 {
                    let len = usize::try_from(
                        decode_varint(&mut body).map_err(|_| TunnelError::Invalid)?,
                    )
                    .map_err(|_| TunnelError::Invalid)?;
                    if len > body.len() {
                        return Err(TunnelError::Invalid);
                    }
                    let (nested, rest) = body.split_at(len);
                    body = rest;
                    let schema = if tag == 3 {
                        &[
                            (1, WireType::LengthDelimited),
                            (2, WireType::LengthDelimited),
                            (3, WireType::Varint),
                        ][..]
                    } else {
                        &[(1, WireType::Varint), (2, WireType::LengthDelimited)][..]
                    };
                    fields(nested, schema, 0, 0)?;
                } else {
                    skip_field(wire, tag, &mut body, DecodeContext::default())
                        .map_err(|_| TunnelError::Invalid)?;
                }
            }
        }
        (39, body) => {
            fields(
                body,
                &[
                    (1, WireType::LengthDelimited),
                    (2, WireType::LengthDelimited),
                    (3, WireType::Varint),
                ],
                0,
                0,
            )?;
        }
        (35, body) => {
            fields(body, &[(1, WireType::Varint)], 0, 0)?;
        }
        (31, body) => {
            let feedback = fields(
                body,
                &[
                    (1, WireType::Varint),
                    (2, WireType::Varint),
                    (3, WireType::Varint),
                    (4, WireType::LengthDelimited),
                    (5, WireType::LengthDelimited),
                ],
                0,
                4,
            )?;
            let state = fields(
                body,
                &[
                    (1, WireType::Varint),
                    (2, WireType::Varint),
                    (3, WireType::Varint),
                    (4, WireType::LengthDelimited),
                    (5, WireType::LengthDelimited),
                ],
                0,
                5,
            )?;
            if let Some(bytes) = state {
                let value = carosign_protocol::carosign::common::v1::CaptureState::decode(bytes)
                    .map_err(|_| TunnelError::Invalid)?;
                if feedback.is_some()
                    || value.encode_to_vec() != bytes
                    || !carosign_protocol::validation::valid_capture_state(&value)
                {
                    return Err(TunnelError::Invalid);
                }
            }
            if let Some(bytes) = feedback {
                let value = carosign_protocol::carosign::common::v1::CaptureFeedback::decode(bytes)
                    .map_err(|_| TunnelError::Invalid)?;
                if value.encode_to_vec() != bytes {
                    return Err(TunnelError::Invalid);
                }
            }
        }
        (32, mut body) => {
            // In evidence, only one option is possible, and the embedded Error and
            // EnrollmentRef They are also checked by the original bytes.
            let mut seen = 0u8;
            let mut evidence = false;
            while !body.is_empty() {
                let (tag, wire) = decode_key(&mut body).map_err(|_| TunnelError::Invalid)?;
                let expected = match tag {
                    1 => WireType::Varint,
                    2..=6 => WireType::LengthDelimited,
                    _ => return Err(TunnelError::Invalid),
                };
                if wire != expected || seen & (1u8 << tag) != 0 {
                    return Err(TunnelError::Invalid);
                }
                seen |= 1u8 << tag;
                if tag == 2 || tag == 3 {
                    if evidence {
                        return Err(TunnelError::Invalid);
                    }
                    evidence = true;
                }
                if tag == 5 || tag == 6 {
                    let len = usize::try_from(
                        decode_varint(&mut body).map_err(|_| TunnelError::Invalid)?,
                    )
                    .map_err(|_| TunnelError::Invalid)?;
                    if len > body.len() {
                        return Err(TunnelError::Invalid);
                    }
                    let (nested, rest) = body.split_at(len);
                    body = rest;
                    let schema = if tag == 5 {
                        &[(1, WireType::Varint), (2, WireType::LengthDelimited)][..]
                    } else {
                        &[
                            (1, WireType::LengthDelimited),
                            (2, WireType::LengthDelimited),
                            (3, WireType::Varint),
                        ][..]
                    };
                    fields(nested, schema, 0, 0)?;
                } else {
                    skip_field(wire, tag, &mut body, DecodeContext::default())
                        .map_err(|_| TunnelError::Invalid)?;
                }
            }
        }
        _ => return Err(TunnelError::Invalid),
    }
    Ok(provider)
}

#[path = "event_validation.tests.rs"]
#[cfg(test)]
mod tests;
