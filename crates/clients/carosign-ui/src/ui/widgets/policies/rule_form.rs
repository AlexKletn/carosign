use carosign_adapters::policy::{Action, Rule};
use carosign_core::operation::context::VerificationPurpose;

pub(super) struct RuleForm {
    pub(super) uid: String,
    pub(super) subject: String,
    pub(super) lifetime: String,
    pub(super) actions: Vec<Action>,
    pub(super) purpose: Option<VerificationPurpose>,
}

pub(super) const ACTIONS: [(Action, &str); 6] = [
    (Action::Verify, "verify"),
    (Action::Enroll, "enroll"),
    (Action::Remove, "remove"),
    (Action::Observe, "observe"),
    (Action::Inventory, "inventory"),
    (Action::Cancel, "cancel"),
];

pub(super) const PURPOSES: [(Option<VerificationPurpose>, &str); 6] = [
    (None, "none"),
    (Some(VerificationPurpose::SessionLogin), "session-login"),
    (Some(VerificationPurpose::SessionUnlock), "session-unlock"),
    (
        Some(VerificationPurpose::PrivilegedAction),
        "privileged-action",
    ),
    (Some(VerificationPurpose::FactorSelfCheck), "self-check"),
    (Some(VerificationPurpose::KeyOperation), "key-operation"),
];

impl Default for RuleForm {
    fn default() -> Self {
        Self {
            uid: String::new(),
            subject: String::new(),
            lifetime: "30".into(),
            actions: vec![],
            purpose: None,
        }
    }
}

impl From<&Rule> for RuleForm {
    fn from(rule: &Rule) -> Self {
        Self {
            uid: rule.uid.to_string(),
            subject: rule.subject.to_string(),
            lifetime: rule.lifetime_s.to_string(),
            actions: rule.actions.clone(),
            purpose: rule.verification_purpose,
        }
    }
}

impl RuleForm {
    pub(super) fn toggle(&mut self, action: Action) {
        if self.actions.contains(&action) {
            self.actions.retain(|existing| *existing != action);
        } else {
            self.actions.push(action);
        }
    }

    pub(super) fn line(&self) -> Option<String> {
        let uid = self.uid.parse::<u32>().ok()?;
        let subject = self.subject.parse::<u64>().ok()?;
        let lifetime = self.lifetime.parse::<u64>().ok()?;
        let mut line = format!("uid={uid} subject={subject} lifetime={lifetime}");
        let actions = ACTIONS
            .iter()
            .filter(|(action, _)| self.actions.contains(action))
            .map(|(_, name)| *name)
            .collect::<Vec<_>>();
        if !actions.is_empty() {
            line.push_str(" actions=");
            line.push_str(&actions.join(","));
        }
        if let Some((_, name)) = PURPOSES
            .iter()
            .find(|(purpose, _)| purpose.is_some() && *purpose == self.purpose)
        {
            line.push_str(" purpose=");
            line.push_str(name);
        }
        Some(line)
    }
}
