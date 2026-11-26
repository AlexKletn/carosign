use core::fmt;

/// Profile is not good.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ProfileError {
    /// There are no rules: such a profile would suit everything.
    NoRules,
    /// Repeat the rule.
    DuplicateRule {
        /// What a sign.
        key: String,
    },
    /// Wrong line.
    Malformed {
        /// Value.
        why: &'static str,
    },
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRules => f.write_str("профиль без правил подошёл бы всему"),
            Self::DuplicateRule { key } => write!(f, "правило {key} повторено"),
            Self::Malformed { why } => write!(f, "профиль неверен: {why}"),
        }
    }
}

impl core::error::Error for ProfileError {}
