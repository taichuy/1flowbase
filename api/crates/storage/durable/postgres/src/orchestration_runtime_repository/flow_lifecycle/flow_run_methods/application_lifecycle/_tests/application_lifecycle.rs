use super::*;
use domain::FlowRunStatus;

#[test]
fn only_initial_running_and_first_terminal_transition_have_application_facts() {
    assert_eq!(transition_phase(None, FlowRunStatus::Queued), None);
    assert_eq!(
        transition_phase(None, FlowRunStatus::Running),
        Some(ApplicationLifecyclePhase::Started)
    );
    assert_eq!(
        transition_phase(Some(FlowRunStatus::Queued), FlowRunStatus::Running),
        Some(ApplicationLifecyclePhase::Started)
    );
    for status in [
        FlowRunStatus::Running,
        FlowRunStatus::WaitingCallback,
        FlowRunStatus::WaitingHuman,
        FlowRunStatus::Paused,
    ] {
        assert_eq!(transition_phase(Some(status), FlowRunStatus::Running), None);
        assert_eq!(transition_phase(Some(FlowRunStatus::Running), status), None);
    }
    for terminal in [
        FlowRunStatus::Succeeded,
        FlowRunStatus::Incomplete,
        FlowRunStatus::Failed,
        FlowRunStatus::Cancelled,
    ] {
        assert_eq!(
            transition_phase(Some(FlowRunStatus::Running), terminal),
            Some(ApplicationLifecyclePhase::Ended)
        );
        assert_eq!(
            transition_phase(Some(FlowRunStatus::WaitingCallback), terminal),
            Some(ApplicationLifecyclePhase::Ended)
        );
        for other in [
            FlowRunStatus::Succeeded,
            FlowRunStatus::Incomplete,
            FlowRunStatus::Failed,
            FlowRunStatus::Cancelled,
        ] {
            assert_eq!(transition_phase(Some(terminal), other), None);
        }
    }
}
