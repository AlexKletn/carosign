use carosign_adapters::policy;

use super::rule_form::RuleForm;

#[derive(Default)]
pub(super) struct PolicyDraft {
    pub(super) baseline: Option<String>,
    pub(super) rules: Vec<RuleForm>,
    pub(super) removed: Option<(usize, RuleForm)>,
    pub(super) selected: usize,
    pub(super) editing: bool,
    pub(super) advanced: bool,
    pub(super) busy: bool,
    pub(super) restarting: bool,
    pub(super) restart_status: Option<&'static str>,
    pub(super) stale: bool,
    pub(super) restart_required: bool,
    pub(super) status: Option<(&'static str, bool)>,
}

impl PolicyDraft {
    pub(super) fn load(&mut self, document: String) -> Result<(), policy::PolicyError> {
        let policy = policy::parse(&document)?;
        self.rules = policy.rules().iter().map(RuleForm::from).collect();
        self.baseline = Some(document);
        self.selected = 0;
        self.editing = false;
        self.advanced = false;
        self.stale = false;
        self.removed = None;
        Ok(())
    }

    pub(super) fn has_changes(&self) -> bool {
        self.baseline
            .as_ref()
            .is_some_and(|baseline| self.document().is_none_or(|document| document != *baseline))
    }

    pub(super) fn document(&self) -> Option<String> {
        let baseline = self.baseline.as_ref()?;
        let mut document = String::new();
        for line in baseline
            .lines()
            .filter(|line| line.trim_start().starts_with('#'))
        {
            document.push_str(line);
            document.push('\n');
        }
        for rule in &self.rules {
            document.push_str(&rule.line()?);
            document.push('\n');
        }
        let proposed = policy::parse(&document).ok()?;
        if policy::parse(baseline).ok().as_ref() == Some(&proposed) {
            return Some(baseline.clone());
        }
        Some(document)
    }
}
