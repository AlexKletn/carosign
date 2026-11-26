/// The context of a private session created by a trusted supervisor.
pub struct Context {
    pub worker: [u8; 16],
    pub epoch: u64,
    pub broker: [u8; 16],
    pub boot: [u8; 16],
}
