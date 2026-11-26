use super::TransactionId;

/// Who is calling and under what request.
///
/// ⚠️ It is collected **only** by a trusted transport adapter from the kernel cradles and
/// from the request number issued by the login system; it cannot be disassembled from the
/// request body (document 08).
///
/// ⚠️ Solution B-04: Same request feature is the transaction number, not the function
/// argument. It gets here when you plug in, lives until you close the connection, and it's
/// verified, and without it, success could have gone to someone else's request.
#[derive(Clone, PartialEq, Eq)]
pub struct CallerContext {
    /// A roughed-up caller's mark: not a name, not a PID, not a path.
    pub opaque: [u8; 32],
    /// UID of kernel creds. ** Field, not tagged byte convention.
    pub uid: u32,
    /// Entry System Request Number (PAM Transaction)
    pub transaction: TransactionId,
}

impl CallerContext {
    /// Gathers context from what the transport and the entry system have given.
    ///
    /// ⚠️ The calling code gets it from the adapter, not from the `ClientFrame`.
    pub fn from_transport(opaque: [u8; 32], uid: u32, transaction: TransactionId) -> Self {
        Self {
            opaque,
            uid,
            transaction,
        }
    }

    /// Context with a clear uid.
    pub fn with_uid(opaque: [u8; 32], uid: u32, transaction: TransactionId) -> Self {
        Self::from_transport(opaque, uid, transaction)
    }

    /// The UID of the caller (as the kernel saw it)
    pub fn uid(&self) -> u32 {
        self.uid
    }

    /// Request number.
    pub fn transaction(&self) -> TransactionId {
        self.transaction
    }

    /// Whether the request number matches that.
    ///
    /// ⚠️ The only legitimate place to answer the question is “is the same request?”
    pub fn answers(&self, transaction: TransactionId) -> bool {
        self.transaction == transaction
    }
}

impl core::fmt::Debug for CallerContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("CallerContext(..)")
    }
}
