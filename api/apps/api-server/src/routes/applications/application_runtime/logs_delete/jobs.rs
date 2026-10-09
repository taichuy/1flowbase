use super::*;
use control_plane_contracts::ports::{
    AgentLogsDeleteJob, AgentLogsDeleteJobCreate, AgentLogsDeletePreview,
};
use serde::Deserialize;

pub(crate) const OPERATIONS: &[&str] = &[
    "applications.logs.delete.jobs.create",
    "applications.logs.delete.jobs.get",
    "applications.logs.delete.jobs.latest",
    "applications.logs.delete.jobs.stop",
];
#[derive(Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(crate) enum JobInput {
    Create {
        application_id: Uuid,
        input: AgentLogsDeleteJobCreate,
    },
    Get {
        application_id: Uuid,
        job_id: Uuid,
    },
    Latest {
        application_id: Uuid,
    },
    Stop {
        application_id: Uuid,
        job_id: Uuid,
    },
}
impl InterfaceContract for JobInput {
    const CONTRACT_ID: &'static str = "console-application-log-deletion-job-input";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        let base = |tag| {
            vec![
                ("action", mp::tag_schema(tag)),
                ("application_id", mp::text_schema()),
            ]
        };
        let mut create = base("create");
        let scope = LogsDeleteInput::managed_projection_schema()?
            .get("properties")?
            .get("scope")?
            .clone();
        create.push((
            "input",
            mp::object_schema(&[("job_id", mp::text_schema()), ("scope", scope)]),
        ));
        let mut get = base("get");
        get.push(("job_id", mp::text_schema()));
        let mut stop = base("stop");
        stop.push(("job_id", mp::text_schema()));
        Some(mp::union_schema(vec![
            mp::object_schema(&create),
            mp::object_schema(&get),
            mp::object_schema(&base("latest")),
            mp::object_schema(&stop),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        let mut value = serde_json::to_value(self).ok()?;
        if let Self::Create {
            application_id,
            input,
        } = self
        {
            value["input"]["scope"] = LogsDeleteInput {
                application_id: *application_id,
                scope: input.scope.clone(),
            }
            .project_for_managed_hook()?["scope"]
                .clone();
        }
        Some(value)
    }
}
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct JobOutput {
    #[schema(value_type=Option<LogsDeletionJobSchema>)]
    pub job: Option<AgentLogsDeleteJob>,
}
struct LogsDeletionJobSchema;
impl utoipa::ToSchema for LogsDeletionJobSchema {}
impl utoipa::PartialSchema for LogsDeletionJobSchema {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        let scope = <super::LogsDeleteScopeSchema as utoipa::PartialSchema>::schema();
        serde_json::from_value(json!({
            "type":"object", "additionalProperties":false,
            "required":["job_id","application_id","scope","ingested_at_before","status","total_records","deleted_records","stop_requested","error_code","created_at","updated_at"],
            "properties":{
                "job_id":{"type":"string","format":"uuid"},
                "application_id":{"type":"string","format":"uuid"},
                "scope":scope,
                "ingested_at_before":{"type":"string","format":"date-time"},
                "status":{"type":"string","enum":["queued","running","succeeded","stopped","failed"]},
                "total_records":{"type":"integer","minimum":0},
                "deleted_records":{"type":"integer","minimum":0},
                "stop_requested":{"type":"boolean"},
                "error_code":{"type":["string","null"]},
                "created_at":{"type":"string","format":"date-time"},
                "updated_at":{"type":"string","format":"date-time"}
            }
        })).expect("static deletion job schema")
    }
}
impl InterfaceContract for JobOutput {
    const CONTRACT_ID: &'static str = "console-application-log-deletion-job-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "job",
            mp::union_schema(vec![
                mp::object_schema(&[
                    ("job_id", mp::text_schema()),
                    ("application_id", mp::text_schema()),
                    (
                        "scope",
                        LogsDeleteInput::managed_projection_schema()?
                            .get("properties")?
                            .get("scope")?
                            .clone(),
                    ),
                    ("ingested_at_before", mp::text_schema()),
                    ("status", mp::text_schema()),
                    ("total_records", mp::count_schema()),
                    ("deleted_records", mp::count_schema()),
                    ("stop_requested", json!({"type":"boolean"})),
                    (
                        "error_code",
                        mp::union_schema(vec![mp::text_schema(), json!({"type":"null"})]),
                    ),
                    ("created_at", mp::text_schema()),
                    ("updated_at", mp::text_schema()),
                ]),
                json!({"type":"null"}),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        let mut value = serde_json::to_value(self).ok()?;
        if let Some(job) = &self.job {
            value["job"]["scope"] = LogsDeleteInput {
                application_id: job.application_id,
                scope: job.scope.clone(),
            }
            .project_for_managed_hook()?["scope"]
                .clone();
        }
        Some(value)
    }
}
struct JobAdapter {
    store: MainDurableStore,
}
impl ConsoleInterfacePort<JobInput, JobOutput> for JobAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: JobInput,
    ) -> ConsoleInterfaceFuture<'a, JobOutput> {
        Box::pin(async move {
            let actor = principal.actor();
            let service = AgentLogsService::new(self.store.for_actor(actor.clone()));
            let job = match input {
                JobInput::Create {
                    application_id,
                    input,
                } => service
                    .start_deletion(actor, application_id, input)
                    .await
                    .map(Some),
                JobInput::Get {
                    application_id,
                    job_id,
                } => {
                    service
                        .deletion_job(actor, application_id, Some(job_id))
                        .await
                }
                JobInput::Latest { application_id } => {
                    service.deletion_job(actor, application_id, None).await
                }
                JobInput::Stop {
                    application_id,
                    job_id,
                } => service
                    .stop_deletion(actor, application_id, job_id)
                    .await
                    .map(Some),
            }
            .map_err(|error| ConsoleInterfaceTargetError(error.into()))?;
            Ok(JobOutput { job })
        })
    }
}
pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.logs.delete.jobs.create",
        binding_id: "http.console.applications.logs.delete.jobs.create.v1",
        method: "POST",
        path: "/api/console/applications/:id/logs/deletion-jobs",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.logs.delete.jobs.get",
        binding_id: "http.console.applications.logs.delete.jobs.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/deletion-jobs/:job_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.logs.delete.jobs.latest",
        binding_id: "http.console.applications.logs.delete.jobs.latest.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/deletion-jobs/latest",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.logs.delete.jobs.stop",
        binding_id: "http.console.applications.logs.delete.jobs.stop.v1",
        method: "POST",
        path: "/api/console/applications/:id/logs/deletion-jobs/:job_id/stop",
        mutating: true,
    },
];
pub(crate) fn compile_registry(
    store: MainDurableStore,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-log-deletion-jobs",
        "graph:console-application-log-deletion-jobs-v1",
        DECLARATIONS,
        Arc::new(JobAdapter { store }),
    )
}

#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct PreviewOutput {
    pub total_records: u64,
}
impl InterfaceContract for PreviewOutput {
    const CONTRACT_ID: &'static str = "console-application-log-deletion-preview-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[("total_records", mp::count_schema())]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(json!({"total_records":self.total_records}))
    }
}
struct PreviewAdapter {
    store: MainDurableStore,
}
impl ConsoleInterfacePort<LogsDeleteInput, PreviewOutput> for PreviewAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: LogsDeleteInput,
    ) -> ConsoleInterfaceFuture<'a, PreviewOutput> {
        Box::pin(async move {
            let actor = principal.actor();
            let AgentLogsDeletePreview { total_records } =
                AgentLogsService::new(self.store.for_actor(actor.clone()))
                    .preview_deletion(actor, input.application_id, input.scope)
                    .await
                    .map_err(|error| ConsoleInterfaceTargetError(error.into()))?;
            Ok(PreviewOutput { total_records })
        })
    }
}
const PREVIEW_DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[ConsoleInterfaceDeclaration {
    interface_id: "applications.logs.delete.preview",
    binding_id: "http.console.applications.logs.delete.preview.v1",
    method: "POST",
    path: "/api/console/applications/:id/logs/deletion-preview",
    mutating: false,
}];
pub(crate) fn compile_preview_registry(
    store: MainDurableStore,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-log-deletion-preview",
        "graph:console-application-log-deletion-preview-v1",
        PREVIEW_DECLARATIONS,
        Arc::new(PreviewAdapter { store }),
    )
}

#[utoipa::path(post,path="/api/console/applications/{id}/logs/deletion-preview",summary="Count imported logs in a deletion scope",request_body(content=inline(super::LogsDeleteScopeSchema)),params(("id"=String,Path)),responses((status=200,body=PreviewOutput)))]
pub(crate) async fn preview(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(scope): Json<AgentLogsDeleteScope>,
) -> Result<Json<ApiSuccess<PreviewOutput>>, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.logs.delete.preview.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        LogsDeleteInput {
            application_id,
            scope,
        },
    )
    .await?;
    Ok(Json(ApiSuccess::new(output)))
}
#[derive(Deserialize, utoipa::ToSchema)]
struct CreateJobSchema {
    job_id: Uuid,
    #[schema(value_type=super::LogsDeleteScopeSchema)]
    scope: AgentLogsDeleteScope,
}
#[utoipa::path(post,path="/api/console/applications/{id}/logs/deletion-jobs",summary="Start a persistent batched log deletion",request_body=CreateJobSchema,params(("id"=String,Path)),responses((status=200,body=JobOutput),(status=409,description="Another deletion is active or job identity conflicts")))]
pub(crate) async fn create(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(input): Json<AgentLogsDeleteJobCreate>,
) -> Result<Json<ApiSuccess<JobOutput>>, ApiError> {
    invoke_job(
        state,
        headers,
        "http.console.applications.logs.delete.jobs.create.v1",
        JobInput::Create {
            application_id,
            input,
        },
        true,
    )
    .await
}
#[utoipa::path(get,path="/api/console/applications/{id}/logs/deletion-jobs/latest",summary="Find the latest log deletion and committed progress",params(("id"=String,Path)),responses((status=200,body=JobOutput)))]
pub(crate) async fn latest(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<JobOutput>>, ApiError> {
    invoke_job(
        state,
        headers,
        "http.console.applications.logs.delete.jobs.latest.v1",
        JobInput::Latest { application_id },
        false,
    )
    .await
}
#[utoipa::path(get,path="/api/console/applications/{id}/logs/deletion-jobs/{job_id}",summary="Read committed log deletion progress",params(("id"=String,Path),("job_id"=String,Path)),responses((status=200,body=JobOutput),(status=404,description="Job not found in this application")))]
pub(crate) async fn get(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<JobOutput>>, ApiError> {
    invoke_job(
        state,
        headers,
        "http.console.applications.logs.delete.jobs.get.v1",
        JobInput::Get {
            application_id,
            job_id,
        },
        false,
    )
    .await
}
#[utoipa::path(post,path="/api/console/applications/{id}/logs/deletion-jobs/{job_id}/stop",summary="Stop a log deletion after its current committed batch",params(("id"=String,Path),("job_id"=String,Path)),responses((status=200,body=JobOutput),(status=404,description="Job not found in this application")))]
pub(crate) async fn stop(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<JobOutput>>, ApiError> {
    invoke_job(
        state,
        headers,
        "http.console.applications.logs.delete.jobs.stop.v1",
        JobInput::Stop {
            application_id,
            job_id,
        },
        true,
    )
    .await
}
async fn invoke_job(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding: &'static str,
    input: JobInput,
    mutating: bool,
) -> Result<Json<ApiSuccess<JobOutput>>, ApiError> {
    let credential = if mutating {
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: state.clone(),
            headers,
        }
    } else {
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: state.clone(),
            headers,
        }
    };
    let output = console_interface::invoke(state, binding, credential, input).await?;
    Ok(Json(ApiSuccess::new(output)))
}
