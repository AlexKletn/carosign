use crate::limits;

use super::WireError;

/// Check the size of the frame before parsing.
pub fn check_frame_size(bytes: &[u8]) -> Result<(), WireError> {
    if bytes.len() > limits::FRAME_MAX {
        return Err(WireError::FrameTooLarge {
            actual: bytes.len(),
        });
    }
    Ok(())
}

/// Verify that the identifier is 16 non-zero bytes.
pub fn require_id(bytes: &[u8], field: &'static str) -> Result<(), WireError> {
    if bytes.len() != limits::ID_LEN || bytes.iter().all(|byte| *byte == 0) {
        return Err(WireError::BadId { field });
    }
    Ok(())
}

/// Verify that the identifier is empty (when the field is optional).
pub fn optional_id(bytes: &[u8], field: &'static str) -> Result<(), WireError> {
    if bytes.is_empty() {
        return Ok(());
    }
    require_id(bytes, field)
}

/// Check the length of a bounded field with a known boundary.
pub fn bounded(value: &[u8], field: &'static str, allowed: usize) -> Result<(), WireError> {
    if value.len() > allowed {
        return Err(WireError::FieldTooLong {
            field,
            actual: value.len(),
            allowed,
        });
    }
    Ok(())
}

/// Check the length of a limited line.
pub fn bounded_str(value: &str, field: &'static str, allowed: usize) -> Result<(), WireError> {
    bounded(value.as_bytes(), field, allowed)
}

/// Checking the non-empty value of the listing.
pub fn specified<E>(value: i32, field: &'static str) -> Result<(), WireError> {
    let _ = core::marker::PhantomData::<E>;
    if value == 0 {
        return Err(WireError::UnspecifiedEnum { field });
    }
    Ok(())
}

/// Check that the listing is known.
pub fn known_enum<E: Copy>(
    value: i32,
    field: &'static str,
    table: &[(i32, E)],
) -> Result<E, WireError> {
    if value == 0 {
        return Err(WireError::UnspecifiedEnum { field });
    }
    table
        .iter()
        .find(|(code, _)| *code == value)
        .map(|(_, item)| *item)
        .ok_or(WireError::UnknownEnum { field, value })
}

/// Checking the sequence of frames in one direction.
///
/// ⚠️ Skipping and repeating is a breakdown of the session, not “getting better”: otherwise
/// you can imperceptibly lose the message about the permission to send.
pub fn check_frame_sequence(expected: u64, actual: u64) -> Result<(), WireError> {
    if expected == 0 || actual != expected {
        return Err(WireError::BadFrameSequence { expected, actual });
    }
    Ok(())
}

pub fn check_version(major: u32, minor: u32) -> Result<(), WireError> {
    if major != limits::MAJOR {
        return Err(WireError::UnsupportedVersion { major, minor });
    }
    Ok(())
}
