use obscur_kit::identity::InternedSymbol;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Page {
    Platform,
    Fingerprints,
    Policies,
    #[default]
    Sections,
}

impl Page {
    pub(crate) const fn command(self) -> InternedSymbol {
        InternedSymbol::named(match self {
            Self::Sections => "show_sections",
            Self::Platform => "show_platform",
            Self::Fingerprints => "show_fingerprints",
            Self::Policies => "show_policies",
        })
    }

    pub(in crate::ui) const fn label(self) -> &'static str {
        match self {
            Self::Sections => "ui-settings-title",
            Self::Platform => "monitor-platform",
            Self::Fingerprints => "ui-registered-fingerprints",
            Self::Policies => "ui-policy-settings",
        }
    }
}
