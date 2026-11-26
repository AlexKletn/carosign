/// Whether to publish.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Readiness {
    /// Ready to receive clients.
    Ready,
    /// Ready, but some areas are closed.
    Degraded,
    /// Not ready: there are areas that no one can judge.
    NotReady,
}
