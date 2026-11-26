use super::*;

pub(super) struct Clock;

impl TimeSource for Clock {
    fn now(&self) -> BootInstant {
        BootInstant::new(BootId::from_bytes([8u8; 16]).expect("валид"), 1_000)
    }
    fn boot_id(&self) -> BootId {
        BootId::from_bytes([8u8; 16]).expect("валид")
    }
}
