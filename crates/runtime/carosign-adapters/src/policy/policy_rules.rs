use super::{Action, PolicyError, Rule, subject_encoding::local_subject};

use carosign_core::identity::SubjectRef;
use carosign_core::ids::AuthorityId;
use carosign_core::operation::context::VerificationPurpose;

/// Discussed politics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Policy {
    rules: Vec<Rule>,
}

/// Discussing politics.
///
/// Line format: `uid=1000 subject=42 actions=verify purpose=session-login lifetime=30`.
/// Without an explicit purpose, the issuance of verification through the system port is
/// prohibited.
pub fn parse(text: &str) -> Result<Policy, PolicyError> {
    let mut policy = Policy::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut uid = None;
        let mut subject = None;
        let mut actions = Vec::new();
        let mut lifetime = 30u64;
        let mut seen_actions = false;
        let mut seen_lifetime = false;
        let mut verification_purpose = None;
        for field in line.split_whitespace() {
            let (key, value) = field.split_once('=').ok_or(PolicyError::Malformed {
                why: "ожидалось ключ=значение",
            })?;
            match key {
                "uid" => {
                    if uid.is_some() {
                        return Err(PolicyError::Malformed {
                            why: "повтор uid"
                        });
                    }
                    uid = Some(
                        value
                            .parse::<u32>()
                            .map_err(|_| PolicyError::Malformed { why: "uid" })?,
                    )
                }
                "subject" => {
                    if subject.is_some() {
                        return Err(PolicyError::Malformed {
                            why: "повтор subject",
                        });
                    }
                    subject = Some(
                        value
                            .parse::<u64>()
                            .map_err(|_| PolicyError::Malformed { why: "subject" })?,
                    )
                }
                "lifetime" => {
                    if seen_lifetime {
                        return Err(PolicyError::Malformed {
                            why: "повтор lifetime",
                        });
                    }
                    seen_lifetime = true;
                    lifetime = value
                        .parse::<u64>()
                        .map_err(|_| PolicyError::Malformed { why: "lifetime" })?
                }
                "actions" => {
                    if seen_actions {
                        return Err(PolicyError::Malformed {
                            why: "повтор actions",
                        });
                    }
                    seen_actions = true;
                    for action in value.split(',') {
                        actions.push(match action {
                            "verify" => Action::Verify,
                            "enroll" => Action::Enroll,
                            "remove" => Action::Remove,
                            "observe" => Action::Observe,
                            "inventory" => Action::Inventory,
                            "cancel" => Action::Cancel,
                            other => return Err(PolicyError::UnknownAction(other.to_string())),
                        });
                    }
                }
                "purpose" => {
                    if verification_purpose.is_some() {
                        return Err(PolicyError::Malformed {
                            why: "повтор purpose",
                        });
                    }
                    verification_purpose = Some(match value {
                        "session-login" => VerificationPurpose::SessionLogin,
                        "session-unlock" => VerificationPurpose::SessionUnlock,
                        "privileged-action" => VerificationPurpose::PrivilegedAction,
                        "self-check" => VerificationPurpose::FactorSelfCheck,
                        "key-operation" => VerificationPurpose::KeyOperation,
                        _ => {
                            return Err(PolicyError::Malformed {
                                why: "неизвестная цель",
                            });
                        }
                    });
                }
                other => return Err(PolicyError::UnknownField(other.to_string())),
            }
        }
        let rule = Rule {
            uid: uid.ok_or(PolicyError::Malformed { why: "нет uid" })?,
            subject: subject.ok_or(PolicyError::Malformed {
                why: "нет subject"
            })?,
            actions,
            lifetime_s: lifetime,
            verification_purpose,
        };
        if policy
            .rules
            .iter()
            .any(|item| item.uid == rule.uid && item.subject == rule.subject)
        {
            return Err(PolicyError::Duplicate);
        }
        policy.rules.push(rule);
    }
    Ok(policy)
}

impl Policy {
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// The target is issued only by an explicitly authorizing rule of the current caller.
    pub fn verification_purpose(
        &self,
        uid: u32,
        subject: SubjectRef,
        authority: AuthorityId,
    ) -> Option<VerificationPurpose> {
        let subject = local_subject(subject, authority)?;
        let rule = self
            .rules
            .iter()
            .find(|rule| rule.uid == uid && rule.subject == subject)?;
        rule.actions
            .contains(&Action::Verify)
            .then_some(rule.verification_purpose)
            .flatten()
    }
    /// Whether the action is permitted.
    pub fn permits(&self, uid: u32, subject: u64, action: Action) -> bool {
        self.rules.iter().any(|rule| {
            rule.uid == uid && rule.subject == subject && rule.actions.contains(&action)
        })
    }

    pub fn permits_for_any_subject(&self, uid: u32, action: Action) -> bool {
        self.rules
            .iter()
            .any(|rule| rule.uid == uid && rule.actions.contains(&action))
    }

    /// How long does the permit live?
    pub fn lifetime_s(&self, uid: u32, subject: u64) -> Option<u64> {
        self.rules
            .iter()
            .find(|rule| rule.uid == uid && rule.subject == subject)
            .map(|rule| rule.lifetime_s)
    }

    pub(super) fn unique_subject(&self, uid: u32) -> Option<u64> {
        let mut rules = self.rules.iter().filter(|rule| {
            rule.uid == uid
                && rule
                    .actions
                    .iter()
                    .any(|action| *action != Action::Inventory)
        });
        let subject = rules.next()?.subject;
        rules.next().is_none().then_some(subject)
    }

    /// It's empty.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}
