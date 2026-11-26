use super::super::profile::Profile;
use super::Candidate;
use core::fmt;

/// How the choice ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// Selected.
    Chosen(Candidate),
    /// Ambiguously, you need a man.
    Ambiguous(Vec<Candidate>),
    /// Nobody came.
    None,
    /// Admin named the one who was not (or did not fit the profile).
    AdminChoiceInvalid(String),
}

/// Picks a candidate.
///
/// ⚠️ Order of priority: list `preferred` It's a policy, not a manifesto.
pub fn resolve(
    installed: &[(String, Profile)],
    inventory: &[(String, String)],
    preferred: &[String],
    admin_choice: Option<&str>,
) -> Resolution {
    let matching: Vec<Candidate> = installed
        .iter()
        .filter(|(_, profile)| super::super::profile::matches(profile, inventory))
        .map(|(package, profile)| Candidate {
            package: package.clone(),
            profile: profile.id.clone(),
        })
        .collect();

    if let Some(choice) = admin_choice {
        let candidates: Vec<_> = matching
            .into_iter()
            .filter(|candidate| candidate.package == choice)
            .collect();
        return match candidates.as_slice() {
            [] => Resolution::AdminChoiceInvalid(choice.to_string()),
            [candidate] => Resolution::Chosen(candidate.clone()),
            _ => Resolution::Ambiguous(candidates),
        };
    }

    if matching.is_empty() {
        return Resolution::None;
    }
    if matching.len() == 1 {
        return Resolution::Chosen(matching[0].clone());
    }

    // Priority: take the first one on the list of policies, if it is among the suitable ones.
    for name in preferred {
        let candidates: Vec<_> = matching
            .iter()
            .filter(|candidate| candidate.package == *name)
            .cloned()
            .collect();
        match candidates.as_slice() {
            [] => {}
            [candidate] => return Resolution::Chosen(candidate.clone()),
            _ => return Resolution::Ambiguous(candidates),
        }
    }

    Resolution::Ambiguous(matching)
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Chosen(candidate) => {
                write!(f, "выбран {} / {}", candidate.package, candidate.profile)
            }
            Self::Ambiguous(list) => write!(f, "неоднозначно: {}", list.len()),
            Self::None => f.write_str("нет подходящих провайдеров"),
            Self::AdminChoiceInvalid(name) => write!(f, "выбор админа недействителен: {name}"),
        }
    }
}

#[path = "resolution.tests.rs"]
#[cfg(test)]
mod tests;
