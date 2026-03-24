//! Email rules/filters — define, persist, and apply rules on incoming mail.

use serde::{Deserialize, Serialize};

use crate::app_state::MailEntry;

/// A single email rule with conditions and actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailRule {
    pub id: String,
    pub name: String,
    pub conditions: Vec<RuleCondition>,
    pub actions: Vec<RuleAction>,
    pub enabled: bool,
}

/// Conditions that can match an incoming email.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum RuleCondition {
    FromContains(String),
    SubjectContains(String),
    ToContains(String),
    HasAttachment,
}

/// Actions to perform when a rule matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum RuleAction {
    MoveToFolder(String),
    MarkAsRead,
    Star,
    Delete,
    AddCategory(String),
}

/// Check whether a mail entry matches all conditions of a rule.
fn matches_rule(mail: &MailEntry, rule: &EmailRule) -> bool {
    if !rule.enabled {
        return false;
    }
    rule.conditions.iter().all(|cond| match cond {
        RuleCondition::FromContains(s) => mail.from.to_lowercase().contains(&s.to_lowercase()),
        RuleCondition::SubjectContains(s) => {
            mail.subject.to_lowercase().contains(&s.to_lowercase())
        }
        RuleCondition::ToContains(s) => mail
            .to
            .iter()
            .any(|t| t.to_lowercase().contains(&s.to_lowercase())),
        RuleCondition::HasAttachment => mail.has_attachments,
    })
}

/// Apply all enabled rules to a mail entry. Returns the list of actions to execute.
pub fn apply_rules(mail: &MailEntry, rules: &[EmailRule]) -> Vec<RuleAction> {
    let mut actions = Vec::new();
    for rule in rules {
        if matches_rule(mail, rule) {
            actions.extend(rule.actions.clone());
        }
    }
    actions
}

// ── Persistence ──────────────────────────────────────────────────────

fn rules_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("exospine")
        .join("rules.json")
}

pub fn load_rules() -> Vec<EmailRule> {
    let path = rules_path();
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_rules(rules: &[EmailRule]) -> Result<(), String> {
    let path = rules_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json =
        serde_json::to_string_pretty(rules).map_err(|e| format!("Failed to serialize: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("Failed to write rules: {}", e))?;
    Ok(())
}
