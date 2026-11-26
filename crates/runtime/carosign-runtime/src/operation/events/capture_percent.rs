/// Percentage of shooting, checked at the border of 0...100.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CapturePercent(u8);

impl CapturePercent {
    /// Checks the value of the provider.
    pub fn new(value: u32) -> Option<Self> {
        if value <= 100 {
            Some(Self(value as u8))
        } else {
            None
        }
    }

    /// Percentage to the client.
    pub fn value(self) -> u8 {
        self.0
    }
}
