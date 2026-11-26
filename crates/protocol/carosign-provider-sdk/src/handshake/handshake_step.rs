/// Allowed handshaking step.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum HandshakeStep {
    /// Send a greeting (without copy ID).
    SendHello,
    /// Wait. `Initialize`.
    AwaitInitialize,
    /// Answer. `Initialized`.
    SendInitialized,
    /// Wait. `BindAuthenticator`.
    AwaitBind,
    /// Answer. `AuthenticatorReady`.
    SendReady,
    /// Take the command.
    AcceptCommand,
}
