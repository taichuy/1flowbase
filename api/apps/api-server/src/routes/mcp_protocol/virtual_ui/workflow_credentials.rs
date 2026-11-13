//! Run references select a host-owned identity; they never authenticate external HTTP requests.
use anyhow::{anyhow, ensure, Context, Result};
use axum::{
    body::{to_bytes, Body},
    http::{HeaderName, HeaderValue, Method, Request, Uri},
    Router,
};
use control_plane_contracts::ports::{
    WorkflowCredentialIdentity, WorkflowCredentialRun, WorkflowRunIdentityRepository,
    WorkflowRunIdentitySnapshot, WorkspaceRepository,
};
use orchestration_runtime::execution_engine::{
    WorkflowCredentialReferences, WorkflowCredentialSubject, WorkflowInternalHttpRequest,
    WorkflowInternalHttpResponse,
};
use tower::ServiceExt;
use uuid::Uuid;

use super::{ApiMcpRuntimeToolInvoker, RuntimeMcpAuthorization};
use crate::middleware::require_session::{with_server_delegated_request_context, RequestContext};
use storage_durable_postgres::MainDurableStore;

impl ApiMcpRuntimeToolInvoker {
    async fn workflow_credential_run(&self, flow_run_id: Uuid) -> Result<WorkflowCredentialRun> {
        let run = self
            .dependencies
            .store
            .get_workflow_credential_run(flow_run_id)
            .await?
            .ok_or_else(|| anyhow!("workflow_credential_run_missing"))?;
        let RuntimeMcpAuthorization::ServerDelegation(delegation) = &self.authorization;
        ensure!(
            !run.status.is_terminal(),
            "workflow_credential_run_terminal"
        );
        ensure!(
            run.executor_user_id == delegation.user_id
                && run.workspace_id == delegation.workspace_id
                && run.tenant_id == delegation.tenant_id,
            "workflow_credential_run_binding_mismatch"
        );
        Ok(run)
    }

    pub(super) async fn prepare_workflow_credential_references(
        &self,
        flow_run_id: Uuid,
    ) -> Result<WorkflowCredentialReferences> {
        let run = self.workflow_credential_run(flow_run_id).await?;
        // A request-scoped repository can hold an actor override. Authentication always reads
        // fresh durable facts rather than using that override's cached permission set.
        let auth_store = MainDurableStore::new(self.dependencies.store.pool().clone());
        let snapshot = match self
            .dependencies
            .store
            .get_workflow_run_identity(flow_run_id)
            .await?
        {
            Some(snapshot) => snapshot,
            None => {
                let RuntimeMcpAuthorization::ServerDelegation(delegation) = &self.authorization;
                let executor = if delegation.active_role_code == "application_api_key" {
                    // Application authority is not a user Console role. Creator selection is
                    // independent, while this executor remains unavailable for Console delegation.
                    None
                } else {
                    let identity = WorkflowCredentialIdentity {
                        user_id: delegation.user_id,
                        tenant_id: delegation.tenant_id,
                        workspace_id: delegation.workspace_id,
                        active_role_code: delegation.active_role_code.clone(),
                        session_version: delegation.session_version,
                    };
                    identity_context(&auth_store, &identity).await?;
                    Some(identity)
                };
                let creator = creator_identity(&auth_store, &run).await?;
                self.dependencies
                    .store
                    .freeze_workflow_run_identity(&WorkflowRunIdentitySnapshot {
                        flow_run_id,
                        creator,
                        executor,
                    })
                    .await?
            }
        };
        validate_snapshot(&run, &snapshot)?;
        // A missing/revoked creator never prevents workflows that use the executor or no credential.
        if let Some(executor) = &snapshot.executor {
            identity_context(&auth_store, executor).await?;
        }
        Ok(WorkflowCredentialReferences::for_run(flow_run_id))
    }

    pub(super) async fn dispatch_workflow_credential_http(
        &self,
        flow_run_id: Uuid,
        credential_reference: &str,
        request: WorkflowInternalHttpRequest,
    ) -> Result<WorkflowInternalHttpResponse> {
        let subject = WorkflowCredentialSubject::from_reference(flow_run_id, credential_reference)?;
        let run = self.workflow_credential_run(flow_run_id).await?;
        let snapshot = self
            .dependencies
            .store
            .get_workflow_run_identity(flow_run_id)
            .await?
            .ok_or_else(|| anyhow!("workflow_run_identity_missing"))?;
        validate_snapshot(&run, &snapshot)?;
        let identity = match subject {
            WorkflowCredentialSubject::Creator => snapshot
                .creator
                .as_ref()
                .ok_or_else(|| anyhow!("workflow_creator_credential_unavailable"))?,
            WorkflowCredentialSubject::Executor => snapshot
                .executor
                .as_ref()
                .ok_or_else(|| anyhow!("workflow_executor_credential_unavailable"))?,
        };
        let auth_store = MainDurableStore::new(self.dependencies.store.pool().clone());
        let context = identity_context(&auth_store, identity).await?;
        control_plane_contracts::ports::with_lifecycle_workflow_ancestors(
            self.dependencies.store.workflow_ancestor_ids(),
            with_server_delegated_request_context(
                context,
                self.interface_dispatch.execute_workflow_http(request),
            ),
        )
        .await
    }
}

fn validate_snapshot(
    run: &WorkflowCredentialRun,
    snapshot: &WorkflowRunIdentitySnapshot,
) -> Result<()> {
    ensure!(
        snapshot.flow_run_id == run.flow_run_id
            && snapshot
                .executor
                .as_ref()
                .is_none_or(|executor| executor.user_id == run.executor_user_id
                    && executor.workspace_id == run.workspace_id
                    && executor.tenant_id == run.tenant_id)
            && snapshot
                .creator
                .as_ref()
                .is_none_or(|creator| creator.user_id == run.application_created_by
                    && creator.workspace_id == run.workspace_id
                    && creator.tenant_id == run.tenant_id),
        "workflow_credential_snapshot_binding_mismatch"
    );
    Ok(())
}

async fn creator_identity(
    store: &MainDurableStore,
    run: &WorkflowCredentialRun,
) -> Result<Option<WorkflowCredentialIdentity>> {
    let Some(user) = store.find_user_by_id(run.application_created_by).await? else {
        return Ok(None);
    };
    if user.status != domain::UserStatus::Active {
        return Ok(None);
    }
    let Some(workspace) = store
        .get_accessible_workspace(user.id, run.workspace_id)
        .await?
    else {
        return Ok(None);
    };
    if workspace.tenant_id != run.tenant_id {
        return Ok(None);
    }
    let role = user
        .default_display_role
        .as_deref()
        .filter(|code| has_bound_role(&user, run.workspace_id, code))
        .or_else(|| {
            user.roles
                .iter()
                .find(|role| {
                    role.scope_kind == domain::RoleScopeKind::System
                        || role.workspace_id == Some(run.workspace_id)
                })
                .map(|role| role.code.as_str())
        });
    let Some(role) = role else {
        return Ok(None);
    };
    Ok(Some(WorkflowCredentialIdentity {
        user_id: user.id,
        tenant_id: run.tenant_id,
        workspace_id: run.workspace_id,
        active_role_code: role.to_string(),
        session_version: user.session_version,
    }))
}

fn has_bound_role(user: &domain::UserRecord, workspace_id: Uuid, role_code: &str) -> bool {
    user.roles.iter().any(|role| {
        role.code == role_code
            && (role.scope_kind == domain::RoleScopeKind::System
                || role.workspace_id == Some(workspace_id))
    })
}

fn validate_user(identity: &WorkflowCredentialIdentity, user: &domain::UserRecord) -> Result<()> {
    ensure!(
        user.id == identity.user_id
            && user.status == domain::UserStatus::Active
            && user.session_version == identity.session_version
            && has_bound_role(user, identity.workspace_id, &identity.active_role_code),
        "workflow_credential_identity_revoked"
    );
    Ok(())
}

async fn identity_context(
    store: &MainDurableStore,
    identity: &WorkflowCredentialIdentity,
) -> Result<RequestContext> {
    let user = store
        .find_user_by_id(identity.user_id)
        .await?
        .ok_or_else(|| anyhow!("workflow_credential_identity_unavailable"))?;
    validate_user(identity, &user)?;
    let workspace = store
        .get_accessible_workspace(user.id, identity.workspace_id)
        .await?
        .ok_or_else(|| anyhow!("workflow_credential_workspace_revoked"))?;
    ensure!(
        workspace.tenant_id == identity.tenant_id,
        "workflow_credential_workspace_mismatch"
    );
    let actor = store
        .load_actor_context(
            user.id,
            identity.tenant_id,
            identity.workspace_id,
            Some(&identity.active_role_code),
        )
        .await?;
    ensure!(
        actor.effective_display_role == identity.active_role_code,
        "workflow_credential_role_revoked"
    );
    Ok(RequestContext::server_delegation(user, actor))
}

fn console_request(request: WorkflowInternalHttpRequest) -> Result<(Request<Body>, usize)> {
    let uri: Uri = request
        .url
        .parse()
        .context("workflow_console_url_invalid")?;
    let decoded = percent_encoding::percent_decode_str(uri.path())
        .decode_utf8()
        .context("workflow_console_url_invalid")?;
    ensure!(
        uri.scheme().is_none()
            && uri.authority().is_none()
            && uri.path().starts_with("/api/console/")
            && decoded.starts_with("/api/console/")
            && !decoded.contains(['\\', '\0', '\r', '\n'])
            && !decoded.split('/').any(|part| matches!(part, "." | ".."))
            && !request.url.contains('#'),
        "workflow_console_url_invalid"
    );
    let method: Method = request
        .method
        .parse()
        .context("workflow_console_method_invalid")?;
    ensure!(
        matches!(
            method.as_str(),
            "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
        ),
        "workflow_console_method_invalid"
    );
    let mut output = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::from(request.body))?;
    for (name, value) in request.headers {
        let name: HeaderName = name.parse()?;
        ensure!(
            !matches!(
                name.as_str(),
                "authorization"
                    | "cookie"
                    | "x-csrf-token"
                    | "host"
                    | "connection"
                    | "transfer-encoding"
                    | "content-length"
                    | "upgrade"
                    | "proxy-authorization"
            ),
            "workflow_console_auth_header_forbidden"
        );
        output
            .headers_mut()
            .append(name, HeaderValue::from_str(&value)?);
    }
    Ok((output, request.max_response_bytes))
}

pub(super) async fn dispatch_console_http(
    router: Router,
    request: WorkflowInternalHttpRequest,
) -> Result<WorkflowInternalHttpResponse> {
    // This entry cannot turn a caller-selected header into a server delegation.
    ensure!(
        crate::middleware::require_session::server_delegated_request_context().is_some(),
        "workflow_console_delegation_required"
    );
    let (request, max_response_bytes) = console_request(request)?;
    let response = router.oneshot(request).await?;
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .filter(|(name, _)| {
            !matches!(
                name.as_str(),
                "set-cookie" | "connection" | "transfer-encoding"
            )
        })
        .map(|(name, value)| Ok((name.to_string(), value.to_str()?.to_string())))
        .collect::<Result<Vec<_>>>()?;
    let body = to_bytes(response.into_body(), max_response_bytes)
        .await?
        .to_vec();
    Ok(WorkflowInternalHttpResponse {
        status,
        headers,
        body,
    })
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "../_tests/workflow_credentials/mod.rs"]
mod tests;
