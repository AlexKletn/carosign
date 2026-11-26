use super::{InvalidStageRecord, limits};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(in super::super) enum TableKind {
    Authenticators = 1,
    Enrollments = 2,
    EnrollmentIndex = 3,
    Mutations = 4,
    MutationScopeLock = 5,
    EffectOutbox = 6,
    Idempotency = 7,
    ProviderState = 8,
    Packages = 9,
    TemplateBlobs = 10,
}

impl TryFrom<u8> for TableKind {
    type Error = InvalidStageRecord;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            1 => Self::Authenticators,
            2 => Self::Enrollments,
            3 => Self::EnrollmentIndex,
            4 => Self::Mutations,
            5 => Self::MutationScopeLock,
            6 => Self::EffectOutbox,
            7 => Self::Idempotency,
            8 => Self::ProviderState,
            9 => Self::Packages,
            10 => Self::TemplateBlobs,
            _ => return Err(InvalidStageRecord),
        })
    }
}

impl TableKind {
    pub(in super::super) fn name(self) -> &'static str {
        match self {
            Self::Authenticators => "authenticators",
            Self::Enrollments => "enrollments",
            Self::EnrollmentIndex => "enrollment_index",
            Self::Mutations => "mutations",
            Self::MutationScopeLock => "mutation_scope_lock",
            Self::EffectOutbox => "effect_outbox",
            Self::Idempotency => "idempotency",
            Self::ProviderState => "provider_state",
            Self::Packages => "packages",
            Self::TemplateBlobs => "template_blobs",
        }
    }

    pub(super) fn key_length_valid(self, key: &[u8]) -> bool {
        let expected = match self {
            Self::Authenticators | Self::Enrollments | Self::Mutations => 16,
            Self::EnrollmentIndex => limits::ENROLLMENT_INDEX_KEY_LEN,
            Self::MutationScopeLock => limits::SCOPE_LOCK_KEY_LEN,
            Self::EffectOutbox => limits::OUTBOX_KEY_LEN,
            Self::Idempotency => limits::IDEMPOTENCY_KEY_LEN,
            Self::ProviderState => 32,
            Self::TemplateBlobs => limits::TEMPLATE_KEY_LEN,
            Self::Packages => {
                return std::str::from_utf8(key)
                    .ok()
                    .and_then(|name| carosign_core::provider::PackageName::new(name).ok())
                    .is_some_and(|name| name.as_str().as_bytes() == key);
            }
        };
        key.len() == expected
    }
    pub(super) fn max_value(self) -> usize {
        let payload = if self == Self::TemplateBlobs {
            limits::TEMPLATE_VALUE_MAX
        } else {
            limits::STORED_VALUE_MAX
        };
        // Staging stores the full original shell, not just payload.
        payload + 8 + limits::DIGEST_LEN
    }
}
