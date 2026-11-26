use super::Profile;

/// Does the profile match the inventory?
///
/// ⚠️ The answer to yes is just "consider this candidate." The answer to no is a final
/// rejection, and you can't guess instead.
pub fn matches(profile: &Profile, inventory: &[(String, String)]) -> bool {
    !profile.matches.is_empty()
        && profile.matches.iter().all(|rule| {
            !rule.key.is_empty() && !rule.value.is_empty() && {
                // Contradictory values of one feature do not allow you to choose a profile by
                // one arbitrary coincidence.
                let mut values = inventory
                    .iter()
                    .filter(|(key, _)| *key == rule.key)
                    .map(|(_, value)| value);
                values
                    .next()
                    .is_some_and(|value| *value == rule.value && values.all(|other| other == value))
            }
        })
}

#[path = "matching.tests.rs"]
#[cfg(test)]
mod matching_tests;
