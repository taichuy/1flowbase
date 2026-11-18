use super::*;

#[test]
fn workflow_references_are_closed_and_bound_to_the_consuming_run() {
    let run = Uuid::from_u128(100);
    let other_run = Uuid::from_u128(101);
    let refs = WorkflowCredentialReferences::for_run(run);
    assert_eq!(
        WorkflowCredentialSubject::from_reference(run, &refs.creator_credential).unwrap(),
        WorkflowCredentialSubject::Creator
    );
    assert_eq!(
        WorkflowCredentialSubject::from_reference(run, &refs.executor_credential).unwrap(),
        WorkflowCredentialSubject::Executor
    );
    for forged in [
        WorkflowCredentialSubject::Creator.reference(other_run),
        format!("{}:executor", refs.creator_credential),
        format!("wfcred:{run}:admin"),
        format!("Bearer {}", refs.executor_credential),
        format!("wfcred:{}:executor", run.simple()),
    ] {
        assert!(
            WorkflowCredentialSubject::from_reference(run, &forged).is_err(),
            "{forged}"
        );
    }
}
