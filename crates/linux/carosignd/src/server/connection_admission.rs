use super::{
    AcceptOutcome, ClientSession, Connection, CountedAcceptError, CountedConnection, IdSource,
    KernelIds, PeerCredentials, PeerUser, sys,
};

/// Registers the socket received in the account. When the socket is rejected, the socket
/// closes; when the shell is destroyed, only the connection is freed up.
#[allow(dead_code)]
pub(crate) fn accept_counted<'a>(
    listener: &std::os::fd::OwnedFd,
    registry: &'a crate::operations::OperationRegistry,
    limit: u8,
    ids: &dyn IdSource,
) -> Result<CountedConnection<'a>, CountedAcceptError> {
    let (fd, peer) = sys::accept_connection(listener)
        .map_err(|error| CountedAcceptError::Accept(AcceptOutcome::Kernel(error.errno)))?;
    let connection = ids
        .new_connection_id()
        .map_err(|_| CountedAcceptError::RandomUnavailable)?;
    let connection = connection_from_peer(fd, peer, *connection.as_bytes())
        .map_err(CountedAcceptError::Accept)?;
    register_counted(connection, registry, limit)
}

#[cfg(test)]
pub(super) fn fresh_connection_id(
    mut source: impl FnMut() -> Result<[u8; 16], sys::SysError>,
) -> Result<[u8; 16], sys::SysError> {
    for _ in 0..8 {
        let bytes = source()?;
        if carosign_core::ids::ConnectionId::from_bytes(bytes).is_ok() {
            return Ok(bytes);
        }
    }
    Err(sys::SysError { errno: libc::EIO })
}

pub(super) fn register_counted(
    connection: Connection,
    registry: &crate::operations::OperationRegistry,
    limit: u8,
) -> Result<CountedConnection<'_>, CountedAcceptError> {
    let handle = registry
        .open_connection(limit)
        .ok_or(CountedAcceptError::AccountingUnavailable)?;
    Ok(CountedConnection {
        connection,
        handle,
        registry,
    })
}

/// It accepts one connection.
pub fn accept_one(
    listener: &std::os::fd::OwnedFd,
    connection: [u8; 16],
) -> Result<Connection, AcceptOutcome> {
    let (fd, peer) =
        sys::accept_connection(listener).map_err(|error| AcceptOutcome::Kernel(error.errno))?;
    connection_from_peer(fd, peer, connection)
}

fn connection_from_peer(
    fd: std::os::fd::OwnedFd,
    peer: PeerUser,
    connection: [u8; 16],
) -> Result<Connection, AcceptOutcome> {
    let credentials = PeerCredentials::from_kernel(peer.pid, peer.uid, peer.gid)
        .map_err(|_| AcceptOutcome::NoCredentials)?;
    Ok(Connection {
        session: ClientSession::new(credentials, connection),
        peer,
        fd,
    })
}

/// New connection number.
pub fn new_connection() -> Result<[u8; 16], carosign_core::ids::IdUnavailable> {
    Ok(*KernelIds::new().new_connection_id()?.as_bytes())
}
