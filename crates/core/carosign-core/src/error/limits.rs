//! Domain limits (document 13, baseline v1).
//!
//! ⚠️ It's **ceilings**: setting up work times can only tighten up.

/// Control message frame, byte.
pub const CONTROL_FRAME_MAX: usize = 65_536;
/// Descriptors in the frame.
pub const FDS_PER_FRAME_MAX: u8 = 8;
/// Non-transparent link provider, byte.
pub const OPAQUE_PROVIDER_REF_MAX: usize = 4_096;
/// Connections simultaneously.
pub const CONNECTIONS_MAX: u32 = 128;
/// Connection operations.
pub const OPS_PER_CONNECTION_MAX: u8 = 16;
/// Operations in line for the machine.
pub const OPS_PER_AUTHENTICATOR_MAX: u8 = 64;
/// Observers for surgery.
pub const OBSERVERS_PER_OPERATION_MAX: u8 = 32;
/// The history of the operation.
pub const EVENT_RING_MAX: u16 = 1_024;
/// The depth of the actor's mail.
pub const ACTOR_MAILBOX_MAX: u16 = 256;
/// Registration tag, UTF-8 byte.
pub const ENROLLMENT_LABEL_MAX: usize = 64;
/// Error text for client, byte UTF-8.
pub const PUBLIC_ERROR_TEXT_MAX: usize = 256;
/// Waiting for Hello from the customer, seconds.
pub const CLIENT_HELLO_TIMEOUT_S: u8 = 5;
/// Waiting for Hello/Initialize from the worker for each phase, seconds.
pub const PROVIDER_HANDSHAKE_TIMEOUT_S: u8 = 5;
/// Waiting. BindAuthenticator, seconds.
pub const PROVIDER_BIND_TIMEOUT_S: u8 = 15;
/// Default check, seconds.
pub const VERIFY_TIMEOUT_S: u8 = 30;
/// Default registration, seconds.
pub const ENROLL_TIMEOUT_S: u8 = 120;
/// Default cleaning, seconds.
pub const CLEANUP_TIMEOUT_S: u8 = 10;
/// Storage pattern, byte.
pub const TEMPLATE_BLOB_MAX: usize = 8 * 1024 * 1024;
/// Metadata recording, byte.
pub const METADATA_VALUE_MAX: usize = 65_536;
/// The request field of the broker handler, byte.
pub const BROKER_PAYLOAD_MAX: usize = 32_768;
