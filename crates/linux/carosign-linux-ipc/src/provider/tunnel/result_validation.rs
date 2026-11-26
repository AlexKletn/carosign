use prost::encoding::{DecodeContext, WireType, decode_key, decode_varint, skip_field};

use super::TunnelError;

// Check the original envelope and enclosed CommandResult To prost: repeating singular/oneof
// or an unknown field should not be a silent success.
pub(super) fn strict_command_result(bytes: &[u8]) -> Result<(), TunnelError> {
    strict_sent_result(bytes, 17)
}

pub(super) fn strict_sent_result(bytes: &[u8], result_tag: u32) -> Result<(), TunnelError> {
    strict_result(bytes, result_tag, 2)
}

pub(super) fn strict_result(
    mut bytes: &[u8],
    result_tag: u32,
    value_tag: u32,
) -> Result<(), TunnelError> {
    let mut seen = 0u32;
    while !bytes.is_empty() {
        let (tag, wire) = decode_key(&mut bytes).map_err(|_| TunnelError::Invalid)?;
        let expected = match tag {
            1 | 2 | 3 | 5 => WireType::Varint,
            4 => WireType::LengthDelimited,
            t if t == result_tag => WireType::LengthDelimited,
            _ => return Err(TunnelError::Invalid),
        };
        if wire != expected || seen & (1u32 << tag) != 0 {
            return Err(TunnelError::Invalid);
        }
        seen |= 1u32 << tag;
        if tag == result_tag {
            let len = usize::try_from(decode_varint(&mut bytes).map_err(|_| TunnelError::Invalid)?)
                .map_err(|_| TunnelError::Invalid)?;
            if len > bytes.len() {
                return Err(TunnelError::Invalid);
            }
            let (mut body, rest) = bytes.split_at(len);
            bytes = rest;
            let mut body_seen = 0u8;
            while !body.is_empty() {
                let (tag, wire) = decode_key(&mut body).map_err(|_| TunnelError::Invalid)?;
                let expected = match tag {
                    1 => WireType::LengthDelimited,
                    t if t == value_tag => WireType::Varint,
                    // The refusal will be maintained after a strict check of the error.
                    _ => return Err(TunnelError::Invalid),
                };
                if wire != expected || body_seen & (1u8 << tag) != 0 {
                    return Err(TunnelError::Invalid);
                }
                body_seen |= 1u8 << tag;
                skip_field(wire, tag, &mut body, DecodeContext::default())
                    .map_err(|_| TunnelError::Invalid)?;
            }
            if body_seen != ((1u8 << 1) | (1u8 << value_tag)) {
                return Err(TunnelError::Invalid);
            }
        } else {
            skip_field(wire, tag, &mut bytes, DecodeContext::default())
                .map_err(|_| TunnelError::Invalid)?;
        }
    }
    if seen & (1u32 << result_tag) == 0 {
        return Err(TunnelError::Invalid);
    }
    Ok(())
}
