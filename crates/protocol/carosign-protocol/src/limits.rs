//! Limits and versions of the wire.
//!
//! ⚠️ These are ** ceilings** schemas (documents 13 and 20). No check can miss more of these
//! numbers, even if the message is collected by us.

/// The upper limit of a serialized frame.
///
/// ⚠️ One frame, one package `SOCK_SEQPACKET`; no length prefix.
pub const FRAME_MAX: usize = 65_536;

/// Descriptors in one shot.
pub const FD_SLOTS_MAX: usize = 8;

/// Opaque provider link and recovery link, byte.
pub const OPAQUE_REF_MAX: usize = 4_096;

/// Non-transparent subject selection, byte.
pub const OPAQUE_SELECTOR_MAX: usize = 256;

/// Reference to the request for authority source, byte.
pub const AUTHORITY_REQUEST_REF_MAX: usize = 256;

/// Registration tag, UTF-8 byte.
pub const LABEL_MAX: usize = 64;

/// Error text for client, byte UTF-8.
pub const SAFE_MESSAGE_MAX: usize = 256;

/// Worker's implementation version, byte.
pub const IMPLEMENTATION_VERSION_MAX: usize = 64;

/// Format identifier, byte.
pub const FORMAT_ID_MAX: usize = 128;

/// The length of the canonical identifier.
pub const ID_LEN: usize = 16;

/// Features in the handshake.
pub const FEATURES_MAX: usize = 32;

/// Devices are in charge.
pub const DEVICES_PER_RESPONSE_MAX: usize = 128;

/// Registration's in charge.
pub const ENROLLMENTS_PER_RESPONSE_MAX: usize = 256;

/// Opaque proof token, byte (personal channel only).
pub const OPAQUE_TOKEN_MAX: usize = 16 * 1024;

/// Payload of the call of the broker, byte.
pub const BROKER_PAYLOAD_MAX: usize = 32 * 1024;

/// The older supported version of the protocol.
pub const MAJOR: u32 = 1;

/// The younger version of baseline.
///
/// ⚠️ The package has not yet been published, so “compatibility with previous releases” is
/// not supported: we understand **only** the current junior (document 34: projection of the
/// registration command and the new version of the package). `planned_enrollment`).
/// When a published consumer appears, there will be two numbers, not one.
pub const MINOR_BASELINE: u32 = 4;

/// It is the upper junior version that we understand.
pub const MINOR_MAX: u32 = 4;

/// The first version transmits the subject's confirmed UID to the provider.
pub const MINOR_SUBJECT_UID: u32 = 4;
