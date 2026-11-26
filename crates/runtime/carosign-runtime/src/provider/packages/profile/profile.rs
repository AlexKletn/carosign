use super::{MatchRule, ProfileError};

/// Profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    /// Profile name.
    pub id: String,
    /// All the rules must be the same (and not OR).
    pub matches: Vec<MatchRule>,
}

/// Deconstructing the profile.
///
/// The format uses an `id = "name"` entry followed by `match.<key> = "value"` entries.
pub fn parse(text: &str) -> Result<Profile, ProfileError> {
    let value: toml::Value = text.parse().map_err(|_| ProfileError::Malformed {
        why: "неверный TOML",
    })?;
    let mut root = value.as_table().cloned().ok_or(ProfileError::Malformed {
        why: "ожидалась таблица профиля",
    })?;
    let id = match root.remove("id") {
        Some(toml::Value::String(id)) if !id.is_empty() => id,
        _ => {
            return Err(ProfileError::Malformed {
                why: "нет идентификатора",
            });
        }
    };
    let rules = match root.remove("match") {
        Some(toml::Value::Table(rules)) => rules,
        None => return Err(ProfileError::NoRules),
        _ => {
            return Err(ProfileError::Malformed {
                why: "правила не являются таблицей",
            });
        }
    };
    if !root.is_empty() {
        return Err(ProfileError::Malformed {
            why: "незнакомое поле профиля",
        });
    }
    let mut matches: Vec<MatchRule> = Vec::new();
    let mut pending = vec![(String::new(), rules)];
    while let Some((prefix, rules)) = pending.pop() {
        for (part, value) in rules {
            // The dots share the characteristics of the inventory: a dot within the component
            // would make different TOML trees the same key.
            if part.is_empty() || part.contains('.') {
                return Err(ProfileError::Malformed {
                    why: "неоднозначный ключ правила",
                });
            }
            let key = if prefix.is_empty() {
                part
            } else {
                format!("{prefix}.{part}")
            };
            match value {
                toml::Value::Table(table) if !table.is_empty() => pending.push((key, table)),
                toml::Value::String(value) if !value.is_empty() => {
                    matches.push(MatchRule { key, value })
                }
                _ => {
                    return Err(ProfileError::Malformed {
                        why: "правило должно быть непустой строкой",
                    });
                }
            }
        }
    }
    if matches.is_empty() {
        return Err(ProfileError::NoRules);
    }

    Ok(Profile { id, matches })
}
