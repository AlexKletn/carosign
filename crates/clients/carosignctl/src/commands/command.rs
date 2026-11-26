use super::CommandError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    ListAuthenticators,
    ListEnrollments {
        /// User (Hint for Source of Authorization)
        user: Option<String>,
    },
    Verify {
        user: String,
    },
    Enroll {
        user: String,
        label: String,
        device: Option<String>,
    },
    Remove {
        enrollment: String,
        user: Option<String>,
    },
    Watch {
        operation: String,
    },
    /// Get a current picture of the operation.
    Status {
        /// Full hexadecimal identifier of the operation.
        operation: String,
    },
    Cancel {
        operation: String,
    },
    /// Health of the service.
    Health,
}

/// Deconstructing a line.
///
/// ⚠️ No command accepts a “fingerprint as a string”: registration is called by a short label,
/// and identification is done by a service.
pub fn parse(words: &[String]) -> Result<Command, CommandError> {
    let Some(head) = words.first() else {
        return Err(CommandError::Unknown {
            word: String::new(),
        });
    };
    let rest = &words[1..];
    let need = |index: usize, what: &'static str| -> Result<String, CommandError> {
        rest.get(index)
            .cloned()
            .ok_or(CommandError::Missing { what })
    };
    let exact = |count: usize| -> Result<(), CommandError> {
        if let Some(word) = rest.get(count) {
            return Err(CommandError::Unknown { word: word.clone() });
        }
        Ok(())
    };

    Ok(match head.as_str() {
        "list" => match rest.first().map(String::as_str) {
            Some("enrollments") => {
                exact(2)?;
                Command::ListEnrollments {
                    user: rest.get(1).cloned(),
                }
            }
            Some(word) => return Err(CommandError::Unknown { word: word.into() }),
            None => Command::ListAuthenticators,
        },
        "verify" => {
            let user = need(0, "пользователь")?;
            exact(1)?;
            Command::Verify { user }
        }
        "enroll" => {
            let user = need(0, "пользователь")?;
            let label = need(1, "метка")?;
            exact(3)?;
            Command::Enroll {
                user,
                label,
                device: rest.get(2).cloned(),
            }
        }
        "remove" => {
            let enrollment = need(0, "регистрация")?;
            exact(2)?;
            Command::Remove {
                enrollment,
                user: rest.get(1).cloned(),
            }
        }
        "watch" => {
            let operation = need(0, "операция")?;
            exact(1)?;
            Command::Watch { operation }
        }
        "status" => {
            let operation = need(0, "операция")?;
            exact(1)?;
            Command::Status { operation }
        }
        "cancel" => {
            let operation = need(0, "операция")?;
            exact(1)?;
            Command::Cancel { operation }
        }
        "health" => {
            exact(0)?;
            Command::Health
        }
        other => {
            return Err(CommandError::Unknown {
                word: other.to_string(),
            });
        }
    })
}

#[path = "command.tests.rs"]
#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests;
