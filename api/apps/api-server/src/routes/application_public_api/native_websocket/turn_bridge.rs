use std::sync::Arc;

use axum::body::Bytes;
use control_plane::application_public_api::{
    callback_resume::{
        PublishedCallbackResumeSource, PublishedCallbackResumeTarget,
        ResumePublishedCallbackCommand,
    },
    native::{
        ApplicationNativeRunService, CancelNativeRunCommand, CreateNativeRunCommand,
        GetNativeRunCommand,
    },
    protocol_translation::TranslationProtocol,
};
use serde_json::{json, Value};
use thiserror::Error;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::{
    projector::NativeWebSocketProjector,
    schema::{sequence_from_event_id, NativeWebSocketClientCommand},
    NativeWebSocketAuthorization,
};
use crate::{
    app_state::ApiState,
    routes::application_public_api::{
        compat_sse::{
            prepare_compatible_resume, start_compatible_typed_attach_stream,
            start_compatible_typed_turn_stream, CompatibleResumeAdmission, PreparedCompatibleTurn,
        },
        native::{
            include_workflow_event_visibility, include_workflow_events, native_error,
            parse_native_run_request,
        },
    },
};

#[derive(Debug, Error)]
pub(crate) enum NativeTurnBridgeError {
    #[error("{message}")]
    Rejected { code: &'static str, message: String },
    #[error("Native runtime stream ended without a terminal event")]
    MissingTerminal,
    #[error("Native WebSocket writer closed")]
    WriterClosed,
}

impl NativeTurnBridgeError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::Rejected { code, .. } => code,
            Self::MissingTerminal => "missing_terminal",
            Self::WriterClosed => "writer_closed",
        }
    }
}

impl From<crate::routes::application_public_api::native::NativeApiError> for NativeTurnBridgeError {
    fn from(error: crate::routes::application_public_api::native::NativeApiError) -> Self {
        Self::Rejected {
            code: error.code,
            message: error.message,
        }
    }
}

pub(crate) struct NativeTurnBridge {
    state: Arc<ApiState>,
    authorization: Arc<NativeWebSocketAuthorization>,
}

impl NativeTurnBridge {
    pub(crate) fn new(
        state: Arc<ApiState>,
        authorization: Arc<NativeWebSocketAuthorization>,
    ) -> Self {
        Self {
            state,
            authorization,
        }
    }

    pub(crate) async fn execute(
        &self,
        command: NativeWebSocketClientCommand,
        frames: mpsc::Sender<String>,
    ) -> Result<(), NativeTurnBridgeError> {
        match command {
            NativeWebSocketClientCommand::Create {
                request_id,
                request,
            } => self.create(request_id, request, frames).await,
            NativeWebSocketClientCommand::Resume {
                request_id,
                run_id,
                callback_task_id,
                response_payload,
                stream_options,
            } => {
                self.resume(
                    request_id,
                    run_id,
                    callback_task_id,
                    response_payload,
                    stream_options,
                    frames,
                )
                .await
            }
            NativeWebSocketClientCommand::Attach {
                request_id,
                run_id,
                after_event_id,
                stream_options,
            } => {
                self.attach(
                    request_id,
                    run_id,
                    after_event_id.as_deref(),
                    stream_options,
                    frames,
                )
                .await
            }
            NativeWebSocketClientCommand::Cancel { .. } => Err(Self::rejected(
                "invalid_command_state",
                "run.cancel is handled by the active connection",
            )),
        }
    }

    pub(crate) async fn cancel(
        &self,
        request_id: &str,
        run_id: Uuid,
    ) -> Result<String, NativeTurnBridgeError> {
        let run = ApplicationNativeRunService::new(self.state.store.clone())
            .with_last_used_cache(self.state.infrastructure.cache_store())
            .with_runtime_event_stream(self.state.runtime_event_stream.clone())
            .cancel_native_run(CancelNativeRunCommand {
                bearer_token: self.authorization.bearer_token.clone(),
                run_id,
            })
            .await
            .map_err(native_error)?;
        Ok(json!({
            "type": "command.accepted",
            "request_id": request_id,
            "run_id": run.id,
            "command": "run.cancel"
        })
        .to_string())
    }

    async fn create(
        &self,
        request_id: String,
        request: Value,
        frames: mpsc::Sender<String>,
    ) -> Result<(), NativeTurnBridgeError> {
        let translated = parse_native_run_request(Bytes::from(
            serde_json::to_vec(&request)
                .map_err(|_| Self::rejected("invalid_request", "request is not serializable"))?,
        ))
        .map_err(|error| Self::rejected(error.code, error.message))?;
        if translated
            .request
            .response_mode
            .as_deref()
            .is_some_and(|mode| mode != "streaming")
        {
            return Err(Self::rejected(
                "response_mode",
                "Native WebSocket requests must omit response_mode or use streaming",
            ));
        }
        let visibility = include_workflow_events(&translated.request)?;
        let run = ApplicationNativeRunService::new(self.state.store.clone())
            .with_last_used_cache(self.state.infrastructure.cache_store())
            .create_native_run(CreateNativeRunCommand {
                bearer_token: self.authorization.bearer_token.clone(),
                request: translated.request,
                protocol: TranslationProtocol::Native,
            })
            .await
            .map_err(native_error)?;
        frames
            .send(
                json!({
                    "type": "run.accepted",
                    "request_id": request_id,
                    "run_id": run.id,
                    "application_id": run.application_id,
                })
                .to_string(),
            )
            .await
            .map_err(|_| NativeTurnBridgeError::WriterClosed)?;
        let stream = start_compatible_typed_turn_stream(
            self.state.clone(),
            PreparedCompatibleTurn::start(run, None, self.authorization.bearer_token.clone()),
        )
        .await?;
        self.project_stream(request_id, visibility, stream, frames)
            .await
    }

    async fn resume(
        &self,
        request_id: String,
        run_id: Uuid,
        callback_task_id: Uuid,
        response_payload: Value,
        stream_options: control_plane::application_public_api::native::NativeStreamOptions,
        frames: mpsc::Sender<String>,
    ) -> Result<(), NativeTurnBridgeError> {
        let command = ResumePublishedCallbackCommand {
            bearer_token: self.authorization.bearer_token.clone(),
            target: PublishedCallbackResumeTarget::FlowRun {
                flow_run_id: run_id,
                callback_task_id,
            },
            source: PublishedCallbackResumeSource::NativeAgent,
            response_payload,
            response_mode: Some("streaming".to_string()),
        };
        let CompatibleResumeAdmission::Resume(plan) =
            prepare_compatible_resume(self.state.clone(), command).await?
        else {
            return Err(Self::rejected(
                "resume_requires_new_turn",
                "this callback must be resumed by creating a new Native turn",
            ));
        };
        let visibility = include_workflow_event_visibility(stream_options.include_workflow_events)?;
        let stream = start_compatible_typed_turn_stream(
            self.state.clone(),
            PreparedCompatibleTurn::resume(plan.initial_run, plan.command),
        )
        .await?;
        self.project_stream(request_id, visibility, stream, frames)
            .await
    }

    async fn attach(
        &self,
        request_id: String,
        run_id: Uuid,
        after_event_id: Option<&str>,
        stream_options: control_plane::application_public_api::native::NativeStreamOptions,
        frames: mpsc::Sender<String>,
    ) -> Result<(), NativeTurnBridgeError> {
        let run = ApplicationNativeRunService::new(self.state.store.clone())
            .with_last_used_cache(self.state.infrastructure.cache_store())
            .get_native_run(GetNativeRunCommand {
                bearer_token: self.authorization.bearer_token.clone(),
                run_id,
            })
            .await
            .map_err(native_error)?;
        let visibility = include_workflow_event_visibility(stream_options.include_workflow_events)?;
        let from_sequence = sequence_from_event_id(run_id, after_event_id)
            .map_err(|error| Self::rejected(error.code(), error.to_string()))?;
        let stream =
            start_compatible_typed_attach_stream(self.state.clone(), run, from_sequence).await?;
        self.project_stream(request_id, visibility, stream, frames)
            .await
    }

    async fn project_stream(
        &self,
        request_id: String,
        visibility: crate::routes::application_public_api::sse::IncludeWorkflowEvents,
        stream: crate::routes::application_public_api::compat_sse::CompatibleTypedTurnStream,
        frames: mpsc::Sender<String>,
    ) -> Result<(), NativeTurnBridgeError> {
        let (_initial_run, mut events) = stream.into_parts();
        let mut projector = NativeWebSocketProjector::new(request_id, visibility);
        while let Some(input) = events.recv().await {
            let (run, envelope) = input.into_parts();
            if let Some(frame) = projector
                .project(&run, envelope)
                .map_err(|error| Self::rejected("projection_failed", error.to_string()))?
            {
                frames
                    .send(frame)
                    .await
                    .map_err(|_| NativeTurnBridgeError::WriterClosed)?;
            }
            if projector.has_terminal() {
                return Ok(());
            }
        }
        Err(NativeTurnBridgeError::MissingTerminal)
    }

    fn rejected(code: &'static str, message: impl Into<String>) -> NativeTurnBridgeError {
        NativeTurnBridgeError::Rejected {
            code,
            message: message.into(),
        }
    }
}
