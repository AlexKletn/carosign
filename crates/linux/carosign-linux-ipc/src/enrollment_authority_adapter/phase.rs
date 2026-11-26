#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Preparing,
    Prepared,
    ChallengePrepared {
        challenge_ref: [u8; 16],
        digest: [u8; 32],
    },
    PlanBound {
        challenge_ref: [u8; 16],
        digest: [u8; 32],
    },
    Validated {
        challenge_ref: [u8; 16],
        digest: [u8; 32],
    },
    Authorized {
        authorization_ref: [u8; 16],
        digest: [u8; 32],
    },
    Bound {
        authorization_ref: [u8; 16],
        digest: [u8; 32],
    },
    Failed,
}
