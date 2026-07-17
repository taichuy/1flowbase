use crate::errors::ControlPlaneError;

pub fn ensure_model_provider_instance_transition(
    from: domain::ModelProviderInstanceStatus,
    to: domain::ModelProviderInstanceStatus,
    action: &'static str,
) -> Result<(), ControlPlaneError> {
    let allowed = matches!(
        (from, to),
        (
            domain::ModelProviderInstanceStatus::Draft,
            domain::ModelProviderInstanceStatus::Draft
        ) | (
            domain::ModelProviderInstanceStatus::Draft,
            domain::ModelProviderInstanceStatus::Ready
        ) | (
            domain::ModelProviderInstanceStatus::Draft,
            domain::ModelProviderInstanceStatus::Invalid
        ) | (
            domain::ModelProviderInstanceStatus::Ready,
            domain::ModelProviderInstanceStatus::Draft
        ) | (
            domain::ModelProviderInstanceStatus::Ready,
            domain::ModelProviderInstanceStatus::Ready
        ) | (
            domain::ModelProviderInstanceStatus::Ready,
            domain::ModelProviderInstanceStatus::Invalid
        ) | (
            domain::ModelProviderInstanceStatus::Invalid,
            domain::ModelProviderInstanceStatus::Draft
        ) | (
            domain::ModelProviderInstanceStatus::Invalid,
            domain::ModelProviderInstanceStatus::Ready
        ) | (
            domain::ModelProviderInstanceStatus::Invalid,
            domain::ModelProviderInstanceStatus::Invalid
        ) | (
            domain::ModelProviderInstanceStatus::Disabled,
            domain::ModelProviderInstanceStatus::Disabled
        )
    );

    if allowed {
        return Ok(());
    }

    Err(invalid_transition(
        "model_provider_instance",
        action,
        from.as_str(),
        to.as_str(),
    ))
}

pub fn ensure_plugin_task_transition(
    from: domain::PluginTaskStatus,
    to: domain::PluginTaskStatus,
    action: &'static str,
) -> Result<(), ControlPlaneError> {
    let allowed = matches!(
        (from, to),
        (
            domain::PluginTaskStatus::Queued,
            domain::PluginTaskStatus::Running
        ) | (
            domain::PluginTaskStatus::Running,
            domain::PluginTaskStatus::Succeeded
        ) | (
            domain::PluginTaskStatus::Running,
            domain::PluginTaskStatus::Failed
        ) | (
            domain::PluginTaskStatus::Running,
            domain::PluginTaskStatus::Canceled
        ) | (
            domain::PluginTaskStatus::Running,
            domain::PluginTaskStatus::TimedOut
        )
    );

    if allowed {
        return Ok(());
    }

    Err(invalid_transition(
        "plugin_task",
        action,
        from.as_str(),
        to.as_str(),
    ))
}

pub fn ensure_flow_run_transition(
    from: domain::FlowRunStatus,
    to: domain::FlowRunStatus,
    action: &'static str,
) -> Result<(), ControlPlaneError> {
    if from == domain::FlowRunStatus::Queued
        && matches!(
            to,
            domain::FlowRunStatus::Failed | domain::FlowRunStatus::Incomplete
        )
    {
        return Ok(());
    }

    let allowed = matches!(
        (from, to),
        (
            domain::FlowRunStatus::Queued,
            domain::FlowRunStatus::Running
        ) | (
            domain::FlowRunStatus::Queued,
            domain::FlowRunStatus::Cancelled
        ) | (
            domain::FlowRunStatus::Running,
            domain::FlowRunStatus::WaitingHuman
        ) | (
            domain::FlowRunStatus::Running,
            domain::FlowRunStatus::WaitingCallback
        ) | (
            domain::FlowRunStatus::Running,
            domain::FlowRunStatus::Succeeded
        ) | (
            domain::FlowRunStatus::Running,
            domain::FlowRunStatus::Incomplete
        ) | (
            domain::FlowRunStatus::Running,
            domain::FlowRunStatus::Failed
        ) | (
            domain::FlowRunStatus::Running,
            domain::FlowRunStatus::Paused
        ) | (
            domain::FlowRunStatus::Running,
            domain::FlowRunStatus::Cancelled
        ) | (
            domain::FlowRunStatus::WaitingHuman,
            domain::FlowRunStatus::WaitingHuman
        ) | (
            domain::FlowRunStatus::WaitingHuman,
            domain::FlowRunStatus::WaitingCallback
        ) | (
            domain::FlowRunStatus::WaitingHuman,
            domain::FlowRunStatus::Succeeded
        ) | (
            domain::FlowRunStatus::WaitingHuman,
            domain::FlowRunStatus::Incomplete
        ) | (
            domain::FlowRunStatus::WaitingHuman,
            domain::FlowRunStatus::Failed
        ) | (
            domain::FlowRunStatus::WaitingHuman,
            domain::FlowRunStatus::Cancelled
        ) | (
            domain::FlowRunStatus::WaitingCallback,
            domain::FlowRunStatus::WaitingCallback
        ) | (
            domain::FlowRunStatus::WaitingCallback,
            domain::FlowRunStatus::WaitingHuman
        ) | (
            domain::FlowRunStatus::WaitingCallback,
            domain::FlowRunStatus::Succeeded
        ) | (
            domain::FlowRunStatus::WaitingCallback,
            domain::FlowRunStatus::Incomplete
        ) | (
            domain::FlowRunStatus::WaitingCallback,
            domain::FlowRunStatus::Failed
        ) | (
            domain::FlowRunStatus::WaitingCallback,
            domain::FlowRunStatus::Cancelled
        ) | (
            domain::FlowRunStatus::Paused,
            domain::FlowRunStatus::Running
        ) | (
            domain::FlowRunStatus::Paused,
            domain::FlowRunStatus::Cancelled
        ) | (
            domain::FlowRunStatus::Paused,
            domain::FlowRunStatus::Incomplete
        )
    );

    if allowed {
        return Ok(());
    }

    Err(invalid_transition(
        "flow_run",
        action,
        from.as_str(),
        to.as_str(),
    ))
}

pub fn ensure_node_run_transition(
    from: domain::NodeRunStatus,
    to: domain::NodeRunStatus,
    action: &'static str,
) -> Result<(), ControlPlaneError> {
    let allowed = matches!(
        (from, to),
        (domain::NodeRunStatus::Pending, domain::NodeRunStatus::Ready)
            | (
                domain::NodeRunStatus::Pending,
                domain::NodeRunStatus::Skipped
            )
            | (
                domain::NodeRunStatus::Pending,
                domain::NodeRunStatus::Failed
            )
            | (domain::NodeRunStatus::Ready, domain::NodeRunStatus::Running)
            | (domain::NodeRunStatus::Ready, domain::NodeRunStatus::Skipped)
            | (domain::NodeRunStatus::Ready, domain::NodeRunStatus::Failed)
            | (
                domain::NodeRunStatus::Running,
                domain::NodeRunStatus::Streaming
            )
            | (
                domain::NodeRunStatus::Running,
                domain::NodeRunStatus::WaitingTool
            )
            | (
                domain::NodeRunStatus::Running,
                domain::NodeRunStatus::WaitingCallback
            )
            | (
                domain::NodeRunStatus::Running,
                domain::NodeRunStatus::WaitingHuman
            )
            | (
                domain::NodeRunStatus::Running,
                domain::NodeRunStatus::Retrying
            )
            | (
                domain::NodeRunStatus::Running,
                domain::NodeRunStatus::Succeeded
            )
            | (
                domain::NodeRunStatus::Running,
                domain::NodeRunStatus::Failed
            )
            | (
                domain::NodeRunStatus::Running,
                domain::NodeRunStatus::Skipped
            )
            | (
                domain::NodeRunStatus::Streaming,
                domain::NodeRunStatus::WaitingTool
            )
            | (
                domain::NodeRunStatus::Streaming,
                domain::NodeRunStatus::WaitingCallback
            )
            | (
                domain::NodeRunStatus::Streaming,
                domain::NodeRunStatus::WaitingHuman
            )
            | (
                domain::NodeRunStatus::Streaming,
                domain::NodeRunStatus::Retrying
            )
            | (
                domain::NodeRunStatus::Streaming,
                domain::NodeRunStatus::Succeeded
            )
            | (
                domain::NodeRunStatus::Streaming,
                domain::NodeRunStatus::Failed
            )
            | (
                domain::NodeRunStatus::WaitingTool,
                domain::NodeRunStatus::Running
            )
            | (
                domain::NodeRunStatus::WaitingTool,
                domain::NodeRunStatus::WaitingCallback
            )
            | (
                domain::NodeRunStatus::WaitingTool,
                domain::NodeRunStatus::WaitingHuman
            )
            | (
                domain::NodeRunStatus::WaitingTool,
                domain::NodeRunStatus::Succeeded
            )
            | (
                domain::NodeRunStatus::WaitingTool,
                domain::NodeRunStatus::Failed
            )
            | (
                domain::NodeRunStatus::WaitingCallback,
                domain::NodeRunStatus::WaitingCallback
            )
            | (
                domain::NodeRunStatus::WaitingCallback,
                domain::NodeRunStatus::WaitingHuman
            )
            | (
                domain::NodeRunStatus::WaitingCallback,
                domain::NodeRunStatus::Succeeded
            )
            | (
                domain::NodeRunStatus::WaitingCallback,
                domain::NodeRunStatus::Failed
            )
            | (
                domain::NodeRunStatus::WaitingHuman,
                domain::NodeRunStatus::WaitingHuman
            )
            | (
                domain::NodeRunStatus::WaitingHuman,
                domain::NodeRunStatus::WaitingCallback
            )
            | (
                domain::NodeRunStatus::WaitingHuman,
                domain::NodeRunStatus::Succeeded
            )
            | (
                domain::NodeRunStatus::WaitingHuman,
                domain::NodeRunStatus::Failed
            )
            | (
                domain::NodeRunStatus::Retrying,
                domain::NodeRunStatus::Running
            )
            | (
                domain::NodeRunStatus::Retrying,
                domain::NodeRunStatus::Succeeded
            )
            | (
                domain::NodeRunStatus::Retrying,
                domain::NodeRunStatus::Failed
            )
    );

    if allowed {
        return Ok(());
    }

    Err(invalid_transition(
        "node_run",
        action,
        from.as_str(),
        to.as_str(),
    ))
}

fn invalid_transition(
    resource: &'static str,
    action: &'static str,
    from: &str,
    to: &str,
) -> ControlPlaneError {
    ControlPlaneError::InvalidStateTransition {
        resource,
        action,
        from: from.to_string(),
        to: to.to_string(),
    }
}
