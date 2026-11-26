/// Where's the session?
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SessionPhase {
    /// Just plugged in, wait. `Hello`.
    AwaitingHello,
    /// Greetings agreed.
    Ready,
    Closed,
}
