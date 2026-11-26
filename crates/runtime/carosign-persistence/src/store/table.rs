/// Scheme tables v1 (document 10).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Table {
    /// Official metadata.
    Meta,
    /// Authentifiers.
    Authenticators,
    /// Registration.
    Enrollments,
    /// Registration index by region.
    EnrollmentIndex,
    /// Change log.
    Mutations,
    /// Blocking areas.
    MutationScopeLock,
    /// A queue of effects.
    EffectOutbox,
    /// Idempotence of requests.
    Idempotency,
    ProviderState,
    Packages,
    /// Templates (ciphertext only).
    TemplateBlobs,
}

impl Table {
    /// Name's in the chart.
    pub fn name(self) -> &'static str {
        match self {
            Self::Meta => "meta",
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
}
