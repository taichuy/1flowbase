#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemAgentIdentity {
    pub actor_kind: String,
    pub agent_name: String,
    pub delegated_user_id: Option<String>,
}

impl SystemAgentIdentity {
    pub fn system(agent_name: impl Into<String>) -> Self {
        Self {
            actor_kind: "system_agent".into(),
            agent_name: agent_name.into(),
            delegated_user_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemAgentAction {
    pub action_kind: String,
    pub risk_level: String,
}

pub fn requires_approval(action: &SystemAgentAction) -> bool {
    matches!(action.risk_level.as_str(), "high" | "critical")
        || matches!(
            action.action_kind.as_str(),
            "write_business_state" | "debit_credit"
        )
}
