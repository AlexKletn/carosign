use super::{Connection, Ended, FRAME_BUFFER, read_frame, write_frame};

/// Reads one message; `None` - the client shut down the connection.
pub(super) fn read_one(connection: &Connection) -> Result<Option<Vec<u8>>, Ended> {
    // ⚠️ A bucket in a pile: `FRAME_MAX` On the handler stack, that's the stack you need for
    // a real job.
    let mut buffer = vec![0u8; FRAME_BUFFER];
    match read_frame(connection, &mut buffer) {
        Ok(Some(len)) => {
            buffer.truncate(len);
            Ok(Some(buffer))
        }
        Ok(None) => Ok(None),
        Err(errno) => Err(Ended::Broken(errno)),
    }
}

/// Sends one whole shot.
pub(super) fn send_one(connection: &Connection, bytes: &[u8]) -> Result<(), Ended> {
    write_frame(connection, bytes).map_err(Ended::Broken)
}
