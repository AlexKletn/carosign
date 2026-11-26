use core::fmt;

/// Manifesto's not good.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ManifestError {
    /// There is no mandatory section.
    MissingSection {
        /// What.
        section: &'static str,
    },
    /// There's no mandatory field.
    MissingField {
        field: &'static str,
    },
    /// Unfamiliar field: the scheme is strict.
    UnknownField {
        /// Where.
        section: &'static str,
        /// Value.
        field: String,
    },
    /// The meaning doesn't fit.
    Invalid {
        /// Where.
        field: &'static str,
        reason: &'static str,
    },
    /// The scheme's wrong.
    UnsupportedSchema {
        /// What's announced.
        found: u32,
    },
    /// The path is dangerous or not relative.
    UnsafePath {
        /// What.
        path: String,
    },
    /// Repeat the ID.
    Duplicate {
        /// That's repeated.
        what: String,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSection { section } => write!(f, "нет раздела [{section}]"),
            Self::MissingField { field } => write!(f, "нет поля {field}"),
            Self::UnknownField { section, field } => {
                write!(f, "незнакомое поле {field} в [{section}]")
            }
            Self::Invalid { field, reason } => write!(f, "поле {field}: {reason}"),
            Self::UnsupportedSchema { found } => write!(f, "схема {found} не поддерживается"),
            Self::UnsafePath { path } => write!(f, "опасный путь {path}"),
            Self::Duplicate { what } => write!(f, "повтор {what}"),
        }
    }
}

impl core::error::Error for ManifestError {}
