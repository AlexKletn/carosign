use super::*;

pub(super) struct ClockState {
    pub(super) last: u64,
    pub(super) failed: bool,
}

impl ClockState {
    pub(super) fn observe(&mut self, reading: Result<u64, SysError>) -> u64 {
        if self.failed {
            return u64::MAX;
        }
        match reading {
            Ok(now) if now >= self.last => {
                self.last = now;
                now
            }
            _ => {
                self.failed = true;
                u64::MAX
            }
        }
    }
}
