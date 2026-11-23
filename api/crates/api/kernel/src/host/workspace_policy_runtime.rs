//! Selects the single active system policy from the same graph as managed authentication.
use crate::{app_state::ApiState, provider_runtime::ApiRuntimeServices};
use control_plane::{
    auth::{WorkspacePolicyContext, WorkspacePolicyPort},
    errors::ControlPlaneError,
};
use extension_contracts::{
    WORKSPACE_POLICY_POINT, WorkspacePolicyRequest, WorkspacePolicyResponse,
};
use runtime_core::runtime_backend::RuntimeExecutionPrincipal;
use std::sync::{Arc, Weak};
use storage_durable_postgres::MainDurableStore;

#[derive(Clone)]
pub struct WorkspacePolicyRuntime(Weak<ApiRuntimeServices>);

impl WorkspacePolicyRuntime {
    pub fn from_state(state: &ApiState) -> Self {
        Self(Arc::downgrade(&state.provider_runtime))
    }

    pub async fn public_onboarding_mode(
        &self,
    ) -> anyhow::Result<Option<extension_contracts::WorkspaceOnboardingModeDto>> {
        match self
            .decide(WorkspacePolicyRequest::InspectPublicPolicy)
            .await?
        {
            None => Ok(None),
            Some(WorkspacePolicyResponse::PublicPolicy { onboarding_mode }) => {
                Ok(Some(onboarding_mode))
            }
            _ => Err(ControlPlaneError::UpstreamUnavailable(
                "workspace_policy_invalid_response",
            )
            .into()),
        }
    }

    pub fn identity_context(&self, store: MainDurableStore) -> WorkspacePolicyContext {
        WorkspacePolicyContext {
            policy: Arc::new(self.clone()),
            repository: Arc::new(store),
        }
    }
}

#[async_trait::async_trait]
impl WorkspacePolicyPort for WorkspacePolicyRuntime {
    async fn decide(
        &self,
        request: WorkspacePolicyRequest,
    ) -> anyhow::Result<Option<WorkspacePolicyResponse>> {
        let runtime = self
            .0
            .upgrade()
            .ok_or(ControlPlaneError::PluginUnavailable)?;
        #[cfg(any(test, feature = "test-support"))]
        if let Some(stub) = &runtime.workspace_policy_fixture {
            return stub.decide(request).await;
        }
        let composition = match runtime.managed_composition() {
            Ok(composition) => composition,
            #[cfg(any(test, feature = "test-support"))]
            Err(_) => return Ok(None),
            #[cfg(not(any(test, feature = "test-support")))]
            Err(error) => return Err(error),
        };
        let Some(snapshot) = composition.snapshot(domain::SYSTEM_SCOPE_ID).await else {
            return Ok(None);
        };
        let mut contributors = snapshot
            .graph
            .points()
            .iter()
            .filter(|point| point.descriptor().point_id.as_str() == WORKSPACE_POLICY_POINT)
            .flat_map(|point| point.contributions());
        let Some(contribution) = contributors.next() else {
            return Ok(None);
        };
        if contributors.next().is_some() {
            return Err(ControlPlaneError::PluginUnavailable.into());
        }
        let id = contribution.descriptor().contribution_id.clone();
        // The trusted principal selects system-owned callbacks and the mounted worker.
        // AdmitOperation's actual target workspace is business data, not worker scope.
        let actor_id = match &request {
            WorkspacePolicyRequest::AdmitOperation { actor_id, .. } => Some(actor_id.clone()),
            WorkspacePolicyRequest::RedeemInvitationForUser { user_id, .. } => {
                Some(user_id.clone())
            }
            _ => None,
        };
        let now = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
        let value = composition
            .execute_snapshot(
                snapshot,
                domain::SYSTEM_SCOPE_ID,
                &id,
                RuntimeExecutionPrincipal {
                    record_permissions: None,
                    caller: None,
                    workspace_id: domain::SYSTEM_SCOPE_ID.to_string(),
                    actor_id,
                    deadline_unix_ms: now as i64 + 15_000,
                },
                serde_json::Value::Null,
                serde_json::to_value(request)?,
            )
            .await
            .map_err(|_| {
                ControlPlaneError::UpstreamUnavailable("workspace_policy_unavailable")
            })?;
        Ok(Some(serde_json::from_value(value).map_err(|_| {
            ControlPlaneError::UpstreamUnavailable("workspace_policy_invalid_response")
        })?))
    }
}

pub async fn admit_workspace_operation(
    policy: &WorkspacePolicyRuntime,
    tenant_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    actor_id: uuid::Uuid,
    interface_id: &str,
) -> anyhow::Result<()> {
    use extension_contracts::WorkspaceOperationAdmissionDto::*;
    match policy
        .decide(WorkspacePolicyRequest::AdmitOperation {
            interface_id: interface_id.into(),
            tenant_id: tenant_id.to_string(),
            workspace_id: workspace_id.to_string(),
            actor_id: actor_id.to_string(),
        })
        .await?
    {
        None | Some(WorkspacePolicyResponse::OperationAdmission { admission: Allow }) => Ok(()),
        Some(WorkspacePolicyResponse::OperationAdmission {
            admission: Deny { reason_code },
        }) => Err(control_plane::auth::workspace_policy_denied(reason_code)),
        _ => Err(
            ControlPlaneError::UpstreamUnavailable("workspace_policy_invalid_response").into(),
        ),
    }
}
