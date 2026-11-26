#[derive(Default)]
pub(super) struct CancelState {
    pub(super) operation: Option<Vec<u8>>,
    pub(super) requested: bool,
    pub(super) sent: bool,
    pub(super) finished: bool,
}
