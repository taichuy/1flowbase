//! One precompiled internal router per generation. Registry descendants only hold Weak edges.
use crate::{
    app_state::ApiState,
    error_response::{ApiError, ApiServiceUnavailable},
    openapi_interface::{
        CallableDispatchError, CallableDispatchForwarding, CallableDispatchPort,
        CallableDispatchResult, DispatchArguments, OpenApiInterfaceCatalogEntry,
    },
    routes::{
        mcp_management::debug_execute::{
            McpDebugExecuteBody, McpDebugExecuteError, McpServerBoundInputs,
        },
        mcp_protocol::virtual_ui::McpInterfaceDispatchPort,
    },
};
use axum::{
    body::{Body, Bytes},
    extract::Request,
    http::HeaderMap,
    response::Response,
    Router,
};
use http_body::{Body as HttpBody, Frame, SizeHint};
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{Arc, OnceLock, Weak},
    task::{Context, Poll},
};

type DispatchFuture<'a> = Pin<
    Box<dyn Future<Output = Result<CallableDispatchResult, CallableDispatchError>> + Send + 'a>,
>;

pub struct GenerationDispatch {
    generation_id: uuid::Uuid,
    #[cfg(any(test, feature = "test-support"))]
    retired: Arc<tokio::sync::Notify>,
    ports: OnceLock<DispatchPorts>,
}

struct DispatchPorts {
    callable: Arc<dyn CallableDispatchPort>,
    mcp: Arc<dyn McpInterfaceDispatchPort>,
}

pub fn unavailable() -> ApiError {
    ApiServiceUnavailable("selected host generation is unavailable").into()
}

impl GenerationDispatch {
    pub fn is_ready(&self) -> bool {
        self.ports.get().is_some()
    }
    pub fn publish_ready(
        &self,
        callable: Arc<dyn CallableDispatchPort>,
        mcp: Arc<dyn McpInterfaceDispatchPort>,
    ) -> Result<(), ApiError> {
        self.ports
            .set(DispatchPorts { callable, mcp })
            .map_err(|_| unavailable())
    }

    pub fn new(generation_id: uuid::Uuid) -> Arc<Self> {
        Arc::new(Self {
            generation_id,
            #[cfg(any(test, feature = "test-support"))]
            retired: Arc::new(tokio::sync::Notify::new()),
            ports: OnceLock::new(),
        })
    }

    #[cfg(any(test, feature = "test-support"))]
    #[doc(hidden)]
    pub fn retirement_notification(&self) -> Arc<tokio::sync::Notify> {
        self.retired.clone()
    }

    pub fn generation_id(&self) -> uuid::Uuid {
        self.generation_id
    }

    pub fn acquire(state: &ApiState) -> Result<Arc<Self>, ApiError> {
        let boot = state
            .extension_boot_snapshot
            .as_ref()
            .ok_or_else(unavailable)?;
        let owner = boot.dispatch_owner()?;
        if owner.generation_id != boot.generation_id() {
            return Err(unavailable());
        }
        Ok(owner)
    }
}

/// The completion task owns its lease independently of the HTTP projection task.
pub fn spawn_with_lease<F>(
    owner: Arc<GenerationDispatch>,
    future: F,
) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(async move {
        let _owner = owner;
        future.await
    })
}

impl CallableDispatchPort for GenerationDispatch {
    fn dispatch<'a>(
        &'a self,
        entry: &'a OpenApiInterfaceCatalogEntry,
        arguments: DispatchArguments,
        injected_path: BTreeMap<String, String>,
        forwarding: CallableDispatchForwarding,
    ) -> DispatchFuture<'a> {
        Box::pin(async move {
            let ports = self
                .ports
                .get()
                .ok_or_else(|| CallableDispatchError::Api(unavailable().0))?;
            ports
                .callable
                .dispatch(entry, arguments, injected_path, forwarding)
                .await
        })
    }
}
#[async_trait::async_trait]
impl McpInterfaceDispatchPort for GenerationDispatch {
    async fn execute_workflow_http(
        &self,
        request: orchestration_runtime::execution_engine::WorkflowInternalHttpRequest,
    ) -> anyhow::Result<orchestration_runtime::execution_engine::WorkflowInternalHttpResponse> {
        let ports = self.ports.get().ok_or_else(|| unavailable().0)?;
        ports.mcp.execute_workflow_http(request).await
    }

    async fn execute(
        &self,
        headers: HeaderMap,
        interface: domain::McpInterfaceCatalogEntry,
        body: McpDebugExecuteBody,
        server_bound_inputs: McpServerBoundInputs,
    ) -> Result<serde_json::Value, McpDebugExecuteError> {
        let ports = self
            .ports
            .get()
            .ok_or_else(|| McpDebugExecuteError::Api(unavailable().0))?;
        ports
            .mcp
            .execute(headers, interface, body, server_bound_inputs)
            .await
    }
}

pub struct WeakGenerationDispatch(pub Weak<GenerationDispatch>);
impl CallableDispatchPort for WeakGenerationDispatch {
    fn dispatch<'a>(
        &'a self,
        entry: &'a OpenApiInterfaceCatalogEntry,
        arguments: DispatchArguments,
        injected_path: BTreeMap<String, String>,
        forwarding: CallableDispatchForwarding,
    ) -> DispatchFuture<'a> {
        Box::pin(async move {
            let owner = self
                .0
                .upgrade()
                .ok_or_else(|| CallableDispatchError::Api(unavailable().0))?;
            owner
                .dispatch(entry, arguments, injected_path, forwarding)
                .await
        })
    }
}
#[async_trait::async_trait]
impl McpInterfaceDispatchPort for WeakGenerationDispatch {
    async fn execute_workflow_http(
        &self,
        request: orchestration_runtime::execution_engine::WorkflowInternalHttpRequest,
    ) -> anyhow::Result<orchestration_runtime::execution_engine::WorkflowInternalHttpResponse> {
        let owner = self.0.upgrade().ok_or_else(|| unavailable().0)?;
        owner.execute_workflow_http(request).await
    }

    async fn execute(
        &self,
        headers: HeaderMap,
        interface: domain::McpInterfaceCatalogEntry,
        body: McpDebugExecuteBody,
        server_bound_inputs: McpServerBoundInputs,
    ) -> Result<serde_json::Value, McpDebugExecuteError> {
        let owner = self
            .0
            .upgrade()
            .ok_or_else(|| McpDebugExecuteError::Api(unavailable().0))?;
        owner
            .execute(headers, interface, body, server_bound_inputs)
            .await
    }
}

/// A host callback selects state and owner together under the publisher's read lock.
pub struct GenerationLease {
    pub state: Arc<ApiState>,
    pub owner: Arc<GenerationDispatch>,
}
impl std::ops::Deref for GenerationLease {
    type Target = Arc<ApiState>;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

/// Only external serving routers own this layer; internal dispatch must stay acyclic.
pub fn serving_router(router: Router, owner: Arc<GenerationDispatch>) -> Router {
    router.layer(axum::middleware::from_fn(
        move |request: Request, next: axum::middleware::Next| {
            let owner = owner.clone();
            async move { bind_response(next.run(request).await, owner) }
        },
    ))
}

pub fn bind_response(response: Response, owner: Arc<GenerationDispatch>) -> Response {
    let (parts, inner) = response.into_parts();
    Response::from_parts(
        parts,
        Body::new(GenerationBody {
            inner,
            owner: Some(owner),
        }),
    )
}
struct GenerationBody {
    inner: Body,
    owner: Option<Arc<GenerationDispatch>>,
}
impl HttpBody for GenerationBody {
    type Data = Bytes;
    type Error = axum::Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let this = self.get_mut();
        let frame = Pin::new(&mut this.inner).poll_frame(cx);
        if matches!(&frame, Poll::Ready(None) | Poll::Ready(Some(Err(_)))) {
            this.owner.take();
        }
        frame
    }
    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Drop for GenerationDispatch {
    fn drop(&mut self) {
        self.retired.notify_one();
    }
}
