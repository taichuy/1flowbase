use super::*;

#[test]
fn running_node_reentry_requires_a_reacquired_claim() {
    let mut owner = crate::ports::RenewResumeClaimInput {
        claim_id: uuid::Uuid::now_v7(),
        claim_token: uuid::Uuid::now_v7(),
        expected_generation: 0,
    };
    assert!(!recovered_running_node_requires_owner_renewal(
        domain::NodeRunStatus::Running,
        None,
    ));
    assert!(!recovered_running_node_requires_owner_renewal(
        domain::NodeRunStatus::Running,
        Some(&owner),
    ));
    owner.expected_generation = 1;
    assert!(recovered_running_node_requires_owner_renewal(
        domain::NodeRunStatus::Running,
        Some(&owner),
    ));
    assert!(!recovered_running_node_requires_owner_renewal(
        domain::NodeRunStatus::WaitingCallback,
        Some(&owner),
    ));
}
