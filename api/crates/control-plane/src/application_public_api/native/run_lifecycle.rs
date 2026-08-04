use super::*;

#[derive(Debug, Clone)]
pub struct CreateNativeRunCommand {
    pub bearer_token: String,
    pub request: NativeRunRequest,
    pub protocol: TranslationProtocol,
}

#[derive(Debug, Clone)]
pub struct GetNativeRunCommand {
    pub bearer_token: String,
    pub run_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct GetNativeRunByProviderResponseIdCommand {
    pub bearer_token: String,
    pub provider_response_id: String,
}

#[derive(Debug, Clone)]
pub struct CancelNativeRunCommand {
    pub bearer_token: String,
    pub run_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeRunValidationError {
    NotAuthenticated,
    ApplicationNotPublished,
    Forbidden,
    NotFound,
    InvalidMapping,
    InvalidToolResults(String),
    InvalidState,
    IdempotencyConflict,
}

pub struct ApplicationNativeRunService<R> {
    repository: R,
    last_used_cache: Option<Arc<dyn CacheStore>>,
    runtime_event_stream: Option<Arc<dyn RuntimeEventStream>>,
}

impl<R> ApplicationNativeRunService<R>
where
    R: ApplicationRepository
        + ApiKeyRepository
        + AuthRepository
        + ApplicationPublicationRepository
        + ApplicationCompiledPlanRepository
        + ApplicationPublishedFlowRunRepository
        + ApplicationPublishedRunControlRepository
        + ApplicationPublishedCallbackAttemptRepository
        + ApplicationPublicConversationRepository
        + Clone,
{
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            last_used_cache: None,
            runtime_event_stream: None,
        }
    }

    pub fn with_last_used_cache(mut self, cache: Arc<dyn CacheStore>) -> Self {
        self.last_used_cache = Some(cache);
        self
    }

    pub fn with_runtime_event_stream(mut self, stream: Arc<dyn RuntimeEventStream>) -> Self {
        self.runtime_event_stream = Some(stream);
        self
    }

    pub async fn create_native_run(
        &self,
        command: CreateNativeRunCommand,
    ) -> std::result::Result<NativeRunResult, NativeRunValidationError> {
        let run = self
            .published_run_service()
            .start_native_run(command)
            .await?;

        Ok(run)
    }

    pub async fn get_native_run(
        &self,
        command: GetNativeRunCommand,
    ) -> std::result::Result<NativeRunResult, NativeRunValidationError> {
        let actor = self
            .api_key_service()
            .authenticate_bearer_token(&command.bearer_token)
            .await
            .map_err(|_| NativeRunValidationError::NotAuthenticated)?;
        let flow_run = self
            .repository
            .get_published_flow_run(command.run_id)
            .await
            .map_err(|_| NativeRunValidationError::NotFound)?
            .ok_or(NativeRunValidationError::NotFound)?;

        if !published_run_belongs_to_actor(&flow_run, actor.application_id, actor.api_key_id) {
            return Err(NativeRunValidationError::Forbidden);
        }

        self.project_published_native_run(actor.application_id, flow_run)
            .await
    }

    pub async fn get_native_run_by_provider_response_id(
        &self,
        command: GetNativeRunByProviderResponseIdCommand,
    ) -> std::result::Result<NativeRunResult, NativeRunValidationError> {
        let actor = self
            .api_key_service()
            .authenticate_bearer_token(&command.bearer_token)
            .await
            .map_err(|_| NativeRunValidationError::NotAuthenticated)?;
        let provider_response_id = command.provider_response_id.trim();
        if provider_response_id.is_empty() {
            return Err(NativeRunValidationError::NotFound);
        }
        let flow_run = self
            .repository
            .find_published_flow_run_by_provider_response_id(
                actor.application_id,
                actor.api_key_id,
                provider_response_id,
            )
            .await
            .map_err(|_| NativeRunValidationError::NotFound)?
            .ok_or(NativeRunValidationError::NotFound)?;
        self.project_published_native_run(actor.application_id, flow_run)
            .await
    }

    async fn project_published_native_run(
        &self,
        application_id: Uuid,
        flow_run: domain::FlowRunRecord,
    ) -> std::result::Result<NativeRunResult, NativeRunValidationError> {
        let metadata = durable_metadata_from_flow_run(&flow_run);
        let initial_run =
            super::super::run_service::native_result_from_flow_run(&flow_run, metadata);
        if let Some(stream_state) = self
            .repository
            .get_published_run_stream_state(application_id, flow_run.id)
            .await
            .map_err(|_| NativeRunValidationError::NotFound)?
        {
            return Ok(
                super::super::run_service::native_result_from_run_stream_state(
                    &initial_run,
                    &stream_state,
                ),
            );
        }

        Ok(initial_run)
    }

    pub async fn cancel_native_run(
        &self,
        command: CancelNativeRunCommand,
    ) -> std::result::Result<NativeRunResult, NativeRunValidationError> {
        let actor = self
            .api_key_service()
            .authenticate_bearer_token(&command.bearer_token)
            .await
            .map_err(|_| NativeRunValidationError::NotAuthenticated)?;

        let flow_run = self
            .repository
            .get_published_flow_run(command.run_id)
            .await
            .map_err(|_| NativeRunValidationError::NotFound)?
            .ok_or(NativeRunValidationError::NotFound)?;
        if !published_run_belongs_to_actor(&flow_run, actor.application_id, actor.api_key_id) {
            return Err(NativeRunValidationError::Forbidden);
        }

        let cancelled = self
            .published_run_service()
            .cancel_published_run(&actor, &flow_run)
            .await?;
        if cancelled.status == domain::FlowRunStatus::Cancelled {
            self.project_committed_cancellation_terminal(&cancelled)
                .await;
            let completed_at = cancelled
                .finished_at
                .unwrap_or_else(OffsetDateTime::now_utc);
            let cancelled_callback_tasks = self
                .repository
                .cancel_published_pending_callback_tasks_for_run(cancelled.id, completed_at)
                .await
                .map_err(|_| NativeRunValidationError::InvalidState)?;
            for callback_task in cancelled_callback_tasks {
                self.repository
                    .append_published_run_event(&crate::ports::AppendRunEventInput {
                        flow_run_id: cancelled.id,
                        node_run_id: Some(callback_task.node_run_id),
                        event_type: "public_run_callback_cancelled".to_string(),
                        payload: json!({
                            "callback_task_id": callback_task.id,
                            "callback_kind": callback_task.callback_kind,
                        }),
                    })
                    .await
                    .map_err(|_| NativeRunValidationError::InvalidMapping)?;
            }
            let cancelled_attempts = self
                .repository
                .cancel_published_callback_resume_attempts_for_run(cancelled.id, completed_at)
                .await
                .map_err(|_| NativeRunValidationError::InvalidState)?;
            for attempt in cancelled_attempts {
                self.repository
                    .append_published_run_event(&crate::ports::AppendRunEventInput {
                        flow_run_id: cancelled.id,
                        node_run_id: None,
                        event_type: "public_run_resume_cancelled".to_string(),
                        payload: json!({
                            "callback_task_id": attempt.callback_task_id,
                            "resume_attempt_id": attempt.id,
                        }),
                    })
                    .await
                    .map_err(|_| NativeRunValidationError::InvalidMapping)?;
            }
        }

        Ok(super::super::run_service::native_result_from_flow_run(
            &cancelled,
            durable_metadata_from_flow_run(&cancelled),
        ))
    }

    fn api_key_service(&self) -> ApplicationApiKeyService<R> {
        let service = ApplicationApiKeyService::new(self.repository.clone());
        match &self.last_used_cache {
            Some(cache) => service.with_last_used_cache(cache.clone()),
            None => service,
        }
    }

    fn published_run_service(&self) -> ApplicationPublishedRunService<R> {
        let service = ApplicationPublishedRunService::new(self.repository.clone());
        match &self.last_used_cache {
            Some(cache) => service.with_last_used_cache(cache.clone()),
            None => service,
        }
    }

    async fn project_committed_cancellation_terminal(&self, flow_run: &domain::FlowRunRecord) {
        let Some(stream) = &self.runtime_event_stream else {
            return;
        };

        // The durable terminal has already won inside `cancel_published_run`.
        // This projection only closes an open live stream; it must not create a
        // second durable terminal record.
        let mut terminal_event =
            crate::orchestration_runtime::debug_stream_events::flow_cancelled(flow_run.id);
        terminal_event.persist_required = false;
        terminal_event.durability = RuntimeEventDurability::Ephemeral;
        if let Err(error) = stream
            .append_terminal_if_missing_and_close(flow_run.id, terminal_event)
            .await
        {
            tracing::warn!(
                flow_run_id = %flow_run.id,
                application_id = %flow_run.application_id,
                error = %error,
                "failed to project committed public cancellation terminal to runtime event stream"
            );
        }
    }
}

#[async_trait]
pub trait NativeRunRepository: Send + Sync {
    async fn create_native_run_result(&self, run: &NativeRunResult) -> Result<NativeRunResult>;
    async fn get_native_run_result(&self, run_id: Uuid) -> Result<Option<NativeRunResult>>;
}
