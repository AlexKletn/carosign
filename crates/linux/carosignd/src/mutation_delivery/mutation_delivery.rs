use super::{
    CallerContext, DeliveryError, MetadataStore, MutationId, OperationId, PreparedMutation, client,
    match_record, terminal,
};

/// Belongs to a single call and is consumed on the first attempt to send.
pub(crate) struct MutationDelivery<'a> {
    prepared: &'a PreparedMutation,
    pub(super) operation: OperationId,
    pub(super) mutation: MutationId,
    pub(super) sequence: u64,
}

impl<'a> MutationDelivery<'a> {
    pub(crate) fn new(
        prepared: &'a PreparedMutation,
        operation: OperationId,
        mutation: MutationId,
        sequence: u64,
    ) -> Result<Self, DeliveryError> {
        if sequence == 0 {
            return Err(DeliveryError::Unavailable);
        }
        Ok(Self {
            prepared,
            operation,
            mutation,
            sequence,
        })
    }

    /// First reads the verified completed journal, then double-checks the authority and
    /// transfers one frame to the closure of the original connection.
    pub(crate) fn send<S: MetadataStore>(
        self,
        store: &S,
        policy: &crate::linux_policy::LinuxPolicy,
        connection: &CallerContext,
        send: impl FnOnce(client::ClientFrame) -> Result<(), ()>,
    ) -> Result<(), DeliveryError> {
        if connection != &self.prepared.caller {
            return Err(DeliveryError::Unavailable);
        }
        let record = carosign_persistence::mutation_store::read_completed_v4(
            store,
            self.mutation,
            self.operation,
        )
        .map_err(|_| DeliveryError::Unavailable)?;
        let (result, enrollment) = match_record(
            &record,
            self.prepared.subject,
            self.prepared.authenticator,
            &self.prepared.plan,
        )?;
        policy
            .revalidate_now(connection, &self.prepared.grant)
            .map_err(|_| DeliveryError::Unavailable)?;
        let frame = terminal(
            self.operation,
            self.mutation,
            self.sequence,
            result,
            enrollment,
        );
        send(frame).map_err(|_| DeliveryError::SendUnknown)
    }
}
