use super::{CallerContext, ClientSession, TransactionId};

pub(crate) fn caller_from_session(session: &ClientSession) -> Option<CallerContext> {
    let credentials = session.peer();
    let mut opaque = [0u8; 32];
    opaque[..4].copy_from_slice(&credentials.pid().to_be_bytes());
    opaque[4..8].copy_from_slice(&credentials.uid().to_be_bytes());
    opaque[31] = 1;
    let transaction = TransactionId::from_bytes(session.connection())?;
    let caller = CallerContext::from_transport(
        opaque,
        credentials.uid(),
        // ⚠️ For a accepted B-04, this is permissible only after providing a separate
        // connection for each PAM call; the connection itself does not confirm identity or
        // authority.
        transaction,
    );
    Some(caller)
}
