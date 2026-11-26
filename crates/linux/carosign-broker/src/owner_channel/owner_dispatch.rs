use super::*;
use carosign_protocol::carosign::provider::v1::ProviderFrame;

impl OwnerSession {
    pub(crate) fn dispatch(
        &mut self,
        request_id: [u8; 16],
        frame: &ProviderFrame,
        timeout: Duration,
    ) -> io::Result<()> {
        if self.terminal
            || self.dispatched
            || !self.requests.is_empty()
            || self.prepare.request_id == request_id
        {
            return Err(invalid());
        }
        self.dispatched = true;
        self.requests.insert(request_id);
        let request = wire::OwnerControl {
            request_id: request_id.to_vec(),
            boot_id: self.prepare.boot_id.clone(),
            scope: self.prepare.scope.clone(),
            dispatch: Some(frame.clone()),
        }
        .encode_to_vec();
        if request.len() > carosign_protocol::limits::FRAME_MAX {
            return Err(invalid());
        }
        let deadline = Instant::now().checked_add(timeout).ok_or_else(invalid)?;
        wait(&self.fd, PollFlags::OUT, deadline)?;
        if send(
            &self.fd,
            &request,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        )? != request.len()
        {
            return Err(io::ErrorKind::WriteZero.into());
        }
        wait(&self.fd, PollFlags::IN, deadline)?;
        let mut bytes = [0u8; 128];
        let mut control = RecvAncillaryBuffer::default();
        let received = recvmsg(
            &self.fd,
            &mut [IoSliceMut::new(&mut bytes)],
            &mut control,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        )?;
        if received.bytes == 0
            || received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        {
            return Err(invalid());
        }
        let raw = &bytes[..received.bytes];
        let reply = wire::DispatchObservation::decode(raw).map_err(|_| invalid())?;
        if reply.encode_to_vec() != raw
            || reply.request_id != request_id
            || reply.command_id != frame.command_id
            || !reply.confirmed
        {
            return Err(invalid());
        }
        Ok(())
    }
}
