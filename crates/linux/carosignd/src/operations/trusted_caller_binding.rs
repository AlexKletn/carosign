/// A link established by a trusted source, not transport data.
/// The production designer will appear along with a proven adapter.
#[allow(dead_code)]
pub(crate) struct TrustedCallerBinding(pub(super) [u8; 32]);

impl TrustedCallerBinding {
    /// Reference to the received connection and kernel credentials.
    pub(crate) fn from_connection(connection: &crate::server::Connection) -> Self {
        let mut bytes = [0u8; 32];
        bytes[..16].copy_from_slice(&connection.session.connection());
        bytes[16..20].copy_from_slice(&connection.peer.uid.to_be_bytes());
        bytes[20..24].copy_from_slice(&connection.peer.gid.to_be_bytes());
        bytes[24..28].copy_from_slice(&connection.peer.pid.to_be_bytes());
        bytes[28..].copy_from_slice(b"unix");
        Self(bytes)
    }
}

#[cfg(test)]
impl TrustedCallerBinding {
    /// A test-only bogus, not a source of authority.
    pub(crate) fn fixture(value: [u8; 32]) -> Self {
        Self(value)
    }
}
