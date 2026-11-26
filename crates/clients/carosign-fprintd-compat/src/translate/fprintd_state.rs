#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FprintdState {
    Idle,
    Swipe,
    Unavailable,
}
