pub(super) enum VerificationBindingStage {
    NotStarted,
    Binding,
    Ready {
        context: carosign_adapters::provider::VerificationFrameContext,
        step: carosign_runtime::operation::actor::Step,
    },
    HandedOff {
        context: carosign_adapters::provider::VerificationFrameContext,
    },
}
