/// What happened to the worker?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorkerEvent {
    /// I came. `WorkerHello`.
    Hello,
    /// I came. `Initialized`.
    Initialized,
    /// I came. `AuthenticatorReady`.
    AuthenticatorReady,
    /// Please clean up (maintenance).
    Quiesce,
    /// The trial's gone.
    Lost,
    /// Error wire or phase.
    ProtocolFault,
}
