use super::Connection;

/// The link is for record only and does not confirm identity or right of access to the
/// transaction.
#[allow(dead_code)]
pub(crate) struct CountedConnection<'a> {
    pub(crate) connection: Connection,
    pub(crate) handle: crate::operations::ConnectionHandle,
    pub(super) registry: &'a crate::operations::OperationRegistry,
}

impl Drop for CountedConnection<'_> {
    fn drop(&mut self) {
        self.registry.close_connection(self.handle);
    }
}
