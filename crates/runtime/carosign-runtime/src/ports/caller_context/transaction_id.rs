/// Transaction number of the login system: 16 non-zero bytes.
///
/// ⚠️ It is issued by the PAM, not by the client, otherwise the same request would be chosen
/// by those who ask.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransactionId([u8; 16]);

impl TransactionId {
    /// Accepts the number, rejecting the zero.
    pub fn from_bytes(bytes: [u8; 16]) -> Option<Self> {
        if bytes == [0u8; 16] {
            return None;
        }
        Some(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl core::fmt::Debug for TransactionId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("TransactionId(..)")
    }
}
