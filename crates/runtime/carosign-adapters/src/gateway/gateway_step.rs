use core::fmt;

/// How did the lock step end?
#[derive(Clone, Debug, PartialEq)]
pub enum GatewayStep {
    /// The transition is made, the effects are performed, the observations are returned to
    /// the actor.
    Progressed {
        /// How many effects have been performed.
        effects: usize,
        /// How many sightings have come back.
        receipts: usize,
    },
    /// There's nothing to do.
    Idle,
    Terminal,
    /// The vault is not available: you can not change, but you can not promise.
    StorageUnavailable,
    /// Rejection: Repeated altering sending or contradictory observation.
    Refused(&'static str),
}

impl fmt::Display for GatewayStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Progressed { effects, receipts } => {
                write!(f, "шаг: эффектов {effects}, наблюдений {receipts}")
            }
            Self::Idle => f.write_str("делать нечего"),
            Self::Terminal => f.write_str("операция завершена"),
            Self::StorageUnavailable => f.write_str("хранилище недоступно"),
            Self::Refused(why) => write!(f, "отказ шлюза: {why}"),
        }
    }
}
