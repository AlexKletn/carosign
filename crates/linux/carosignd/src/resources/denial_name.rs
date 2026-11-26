use super::ResourceDenial;

/// Refusal of resources in the form of a word for the journal.
pub fn denial_name(denial: ResourceDenial) -> &'static str {
    match denial {
        ResourceDenial::Busy => "busy",
        ResourceDenial::Denied => "denied",
        ResourceDenial::Unavailable => "unavailable",
    }
}
