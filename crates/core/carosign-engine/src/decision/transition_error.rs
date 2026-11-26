use crate::transition::OperationPhase;
use carosign_core::modality::OperationKind;
use core::fmt;

/// Entrance is not consistent with the state - transition is impossible.
///
/// This error denotes a race between the engine and runtime rather than an operation outcome
/// or an execution failure.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TransitionError {
    /// Entry is not allowed in the current phase.
    UnexpectedInput {
        /// The phase in which the entrance came.
        phase: OperationPhase,
    },
    /// The entrance is a different type of operation (C-03).
    ///
    /// ⚠️ Sending. `SendVerify` c the registration machine is this refusal: there is no
    /// transition, there are no side effects.
    InvalidInputForOperation {
        /// The kind of operation that the entrance came in.
        kind: OperationKind,
        /// What kind of entrance belongs to.
        belongs_to: OperationKind,
    },
    /// The operation is complete.
    AlreadyTerminal,
    /// Entrance from an alien worker or an outdated era.
    StaleFencing,
    /// Confirmation is not about that effect.
    ReceiptMismatch,
    /// Transition for which there is no accepted table.
    NotSpecified {
        /// What's not defined.
        what: &'static str,
    },
    /// The transition index is overflowing.
    TransitionIndexOverflow,
}

impl fmt::Debug for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedInput { phase } => {
                write!(f, "TransitionError::UnexpectedInput({phase:?})")
            }
            Self::InvalidInputForOperation { kind, belongs_to } => write!(
                f,
                "TransitionError::InvalidInputForOperation({kind:?} ← {belongs_to:?})"
            ),
            Self::AlreadyTerminal => f.write_str("TransitionError::AlreadyTerminal"),
            Self::StaleFencing => f.write_str("TransitionError::StaleFencing"),
            Self::ReceiptMismatch => f.write_str("TransitionError::ReceiptMismatch"),
            Self::NotSpecified { what } => write!(f, "TransitionError::NotSpecified({what})"),
            Self::TransitionIndexOverflow => {
                f.write_str("TransitionError::TransitionIndexOverflow")
            }
        }
    }
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedInput { phase } => write!(f, "вход недопустим в фазе {phase:?}"),
            Self::InvalidInputForOperation { kind, belongs_to } => write!(
                f,
                "вход вида {belongs_to:?} не относится к операции {kind:?}"
            ),
            Self::AlreadyTerminal => f.write_str("операция уже завершена"),
            Self::StaleFencing => f.write_str("вход от устаревшего работника или эпохи"),
            Self::ReceiptMismatch => {
                f.write_str("подтверждение не соответствует ожидаемому эффекту")
            }
            Self::NotSpecified { what } => write!(f, "переход не определён документом: {what}"),
            Self::TransitionIndexOverflow => f.write_str("счётчик переходов переполнен"),
        }
    }
}

impl core::error::Error for TransitionError {}
