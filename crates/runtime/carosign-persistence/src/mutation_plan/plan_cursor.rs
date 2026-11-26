use super::PlanError;

pub(super) struct PlanCursor<'a> {
    pub(super) data: &'a [u8],
    pub(super) at: usize,
}

impl<'a> PlanCursor<'a> {
    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8], PlanError> {
        if self.data.len().saturating_sub(self.at) < count {
            return Err(PlanError::DigestMismatch);
        }
        let out = &self.data[self.at..self.at + count];
        self.at += count;
        Ok(out)
    }

    pub(super) fn arr16(&mut self) -> Result<[u8; 16], PlanError> {
        let mut out = [0u8; 16];
        out.copy_from_slice(self.take(16)?);
        Ok(out)
    }

    pub(super) fn u32(&mut self) -> Result<u32, PlanError> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub(super) fn u64(&mut self) -> Result<u64, PlanError> {
        let bytes = self.take(8)?;
        let mut out = [0u8; 8];
        out.copy_from_slice(bytes);
        Ok(u64::from_be_bytes(out))
    }
}
