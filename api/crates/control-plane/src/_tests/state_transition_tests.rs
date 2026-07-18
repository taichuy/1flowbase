use crate::{
    errors::ControlPlaneError,
    state_transition::{
        ensure_flow_run_transition, ensure_model_provider_instance_transition,
        ensure_node_run_transition, ensure_plugin_task_transition,
    },
};
use domain::{FlowRunStatus, ModelProviderInstanceStatus, NodeRunStatus, PluginTaskStatus};

#[test]
fn model_provider_instance_allows_expected_edit_and_validation_cycles() {
    assert!(ensure_model_provider_instance_transition(
        ModelProviderInstanceStatus::Draft,
        ModelProviderInstanceStatus::Ready,
        "validate_instance_success",
    )
    .is_ok());
    assert!(ensure_model_provider_instance_transition(
        ModelProviderInstanceStatus::Ready,
        ModelProviderInstanceStatus::Draft,
        "update_instance",
    )
    .is_ok());
    assert!(ensure_model_provider_instance_transition(
        ModelProviderInstanceStatus::Invalid,
        ModelProviderInstanceStatus::Ready,
        "validate_instance_success",
    )
    .is_ok());
    assert!(ensure_model_provider_instance_transition(
        ModelProviderInstanceStatus::Disabled,
        ModelProviderInstanceStatus::Disabled,
        "update_instance",
    )
    .is_ok());
}

#[test]
fn model_provider_instance_rejects_disabled_validation() {
    let error = ensure_model_provider_instance_transition(
        ModelProviderInstanceStatus::Disabled,
        ModelProviderInstanceStatus::Ready,
        "validate_instance_success",
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ControlPlaneError::InvalidStateTransition { resource, action, from, to }
            if resource == "model_provider_instance"
                && action == "validate_instance_success"
                && from == "disabled"
                && to == "ready"
    ));
}

#[test]
fn plugin_task_rejects_restarting_terminal_status() {
    let error = ensure_plugin_task_transition(
        PluginTaskStatus::Succeeded,
        PluginTaskStatus::Running,
        "plugin_task_progress",
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ControlPlaneError::InvalidStateTransition { resource, from, to, .. }
            if resource == "plugin_task" && from == "succeeded" && to == "running"
    ));
}

#[test]
fn flow_run_allows_queued_shell_to_fail_during_preparation() {
    assert!(ensure_flow_run_transition(
        FlowRunStatus::Queued,
        FlowRunStatus::Failed,
        "fail_flow_run",
    )
    .is_ok());
}

#[test]
fn flow_run_rejects_terminal_resume() {
    let error = ensure_flow_run_transition(
        FlowRunStatus::Succeeded,
        FlowRunStatus::Running,
        "resume_flow_run",
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ControlPlaneError::InvalidStateTransition { resource, from, to, .. }
            if resource == "flow_run" && from == "succeeded" && to == "running"
    ));
}

#[test]
fn d1_ac_007_flow_run_incomplete_is_terminal_and_cannot_be_reclassified() {
    assert!(ensure_flow_run_transition(
        FlowRunStatus::Running,
        FlowRunStatus::Incomplete,
        "persist_flow_incomplete",
    )
    .is_ok());

    for next_status in [
        FlowRunStatus::Succeeded,
        FlowRunStatus::Failed,
        FlowRunStatus::Cancelled,
    ] {
        let error = ensure_flow_run_transition(
            FlowRunStatus::Incomplete,
            next_status,
            "replace_incomplete_terminal",
        )
        .expect_err("D1-AC-007: incomplete must remain its own terminal result");

        assert!(matches!(
            error,
            ControlPlaneError::InvalidStateTransition { resource, from, to, .. }
                if resource == "flow_run" && from == "incomplete" && to == next_status.as_str()
        ));
    }
}

#[test]
fn node_run_rejects_promoting_failed_run_back_to_success() {
    let error = ensure_node_run_transition(
        NodeRunStatus::Failed,
        NodeRunStatus::Succeeded,
        "resume_waiting_node",
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ControlPlaneError::InvalidStateTransition { resource, from, to, .. }
            if resource == "node_run" && from == "failed" && to == "succeeded"
    ));
}
