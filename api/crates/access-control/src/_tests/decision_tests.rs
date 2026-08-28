use std::collections::BTreeSet;

use crate::{
    aggregate_authorization_decisions, AuthorizationConstraint, AuthorizationDecision,
    AuthorizationDecisionError, AuthorizationDenyReason, EffectiveAuthorization,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct AllowedFields(BTreeSet<&'static str>);

impl AuthorizationConstraint for AllowedFields {
    type Error = std::convert::Infallible;

    fn intersect(&self, other: &Self) -> Result<Self, Self::Error> {
        Ok(Self(self.0.intersection(&other.0).copied().collect()))
    }
}

#[test]
fn lcf_003_deny_is_absorbing_and_permutation_independent() {
    let deny = AuthorizationDecision::Deny(AuthorizationDenyReason::new("blocked").unwrap());
    let allow = AuthorizationDecision::Allow;
    let first =
        aggregate_authorization_decisions::<AllowedFields>(vec![deny.clone(), allow.clone()])
            .unwrap();
    let second = aggregate_authorization_decisions::<AllowedFields>(vec![allow, deny]).unwrap();

    assert_eq!(first, second);
    assert!(matches!(first, EffectiveAuthorization::Denied(_)));
}

#[test]
fn lcf_003_constraints_use_domain_owned_safe_intersection() {
    let first = AllowedFields(BTreeSet::from(["id", "name"]));
    let second = AllowedFields(BTreeSet::from(["name", "email"]));
    let effective = aggregate_authorization_decisions(vec![
        AuthorizationDecision::Constraint(first),
        AuthorizationDecision::Constraint(second),
    ])
    .unwrap();

    assert_eq!(
        effective,
        EffectiveAuthorization::Constrained(AllowedFields(BTreeSet::from(["name"])))
    );
}

#[test]
fn lcf_004_empty_security_handler_set_fails_closed() {
    assert_eq!(
        aggregate_authorization_decisions::<AllowedFields>(Vec::new()),
        Err(AuthorizationDecisionError::EmptyDecisionSet)
    );
}
