use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationDenyReason(String);

impl AuthorizationDenyReason {
    pub fn new(value: impl Into<String>) -> Result<Self, AuthorizationDecisionError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(AuthorizationDecisionError::EmptyDenyReason);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub trait AuthorizationConstraint: Clone + Send + Sync + 'static {
    type Error: fmt::Display;

    fn intersect(&self, other: &Self) -> Result<Self, Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationDecision<C> {
    Allow,
    Deny(AuthorizationDenyReason),
    Constraint(C),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectiveAuthorization<C> {
    Allowed,
    Denied(AuthorizationDenyReason),
    Constrained(C),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationDecisionError {
    EmptyDecisionSet,
    EmptyDenyReason,
    ConstraintIntersection(String),
}

impl fmt::Display for AuthorizationDecisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDecisionSet => formatter.write_str("authorization decision set is empty"),
            Self::EmptyDenyReason => formatter.write_str("authorization deny reason is empty"),
            Self::ConstraintIntersection(error) => {
                write!(
                    formatter,
                    "authorization constraints cannot be intersected: {error}"
                )
            }
        }
    }
}

impl std::error::Error for AuthorizationDecisionError {}

pub fn aggregate_authorization_decisions<C>(
    decisions: impl IntoIterator<Item = AuthorizationDecision<C>>,
) -> Result<EffectiveAuthorization<C>, AuthorizationDecisionError>
where
    C: AuthorizationConstraint,
{
    let mut saw_decision = false;
    let mut constraint: Option<C> = None;
    let mut deny: Option<AuthorizationDenyReason> = None;

    for decision in decisions {
        saw_decision = true;
        match decision {
            AuthorizationDecision::Allow => {}
            AuthorizationDecision::Deny(reason) => {
                if deny
                    .as_ref()
                    .is_none_or(|current| reason.as_str() < current.as_str())
                {
                    deny = Some(reason);
                }
            }
            AuthorizationDecision::Constraint(incoming) => {
                constraint = Some(match constraint {
                    Some(current) => current.intersect(&incoming).map_err(|error| {
                        AuthorizationDecisionError::ConstraintIntersection(error.to_string())
                    })?,
                    None => incoming,
                });
            }
        }
    }

    if !saw_decision {
        return Err(AuthorizationDecisionError::EmptyDecisionSet);
    }
    if let Some(reason) = deny {
        return Ok(EffectiveAuthorization::Denied(reason));
    }
    Ok(match constraint {
        Some(constraint) => EffectiveAuthorization::Constrained(constraint),
        None => EffectiveAuthorization::Allowed,
    })
}
