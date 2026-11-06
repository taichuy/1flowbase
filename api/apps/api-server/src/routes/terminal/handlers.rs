use super::*;
/// Inspect backend terminal capabilities.
#[utoipa::path(post, path = "/api/console/terminal/info", operation_id = "terminal.info", request_body = InfoInput, responses((status = 200, body = InfoOutput), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn info(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<InfoInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.info.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::Info(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
/// List local directory entries.
#[utoipa::path(post, path = "/api/console/terminal/files/list", operation_id = "terminal.files.list", request_body = ListInput, responses((status = 200, body = ListOutput), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn files_list(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ListInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.files.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::List(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
/// Inspect a local file.
#[utoipa::path(post, path = "/api/console/terminal/files/stat", operation_id = "terminal.files.stat", request_body = PathInput, responses((status = 200, body = FileMetadata), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn files_stat(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<PathInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.files.stat.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::Stat(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
/// Read a local text slice.
#[utoipa::path(post, path = "/api/console/terminal/files/read-text", operation_id = "terminal.files.read_text", request_body = TextInput, responses((status = 200, body = TextOutput), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn files_read_text(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<TextInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.files.read_text.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::Text(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
/// Read a local byte range.
#[utoipa::path(post, path = "/api/console/terminal/files/read-range", operation_id = "terminal.files.read_range", request_body = RangeInput, responses((status = 200, body = RangeOutput), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn files_read_range(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<RangeInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.files.read_range.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::Range(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
/// Search local text files.
#[utoipa::path(post, path = "/api/console/terminal/files/search", operation_id = "terminal.files.search", request_body = SearchInput, responses((status = 200, body = SearchOutput), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn files_search(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<SearchInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.files.search.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::Search(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
/// Read a local image.
#[utoipa::path(post, path = "/api/console/terminal/files/read-image", operation_id = "terminal.files.read_image", request_body = MediaInput, responses((status = 200, body = MediaOutput), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn files_read_image(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<MediaInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.files.read_image.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::Image(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
/// Read local audio.
#[utoipa::path(post, path = "/api/console/terminal/files/read-audio", operation_id = "terminal.files.read_audio", request_body = MediaInput, responses((status = 200, body = MediaOutput), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn files_read_audio(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<MediaInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.files.read_audio.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::Audio(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
/// Read a local resource.
#[utoipa::path(post, path = "/api/console/terminal/files/read-resource", operation_id = "terminal.files.read_resource", request_body = ResourceInput, responses((status = 200, body = ResourceOutput), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn files_read_resource(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ResourceInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.files.read_resource.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::Resource(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
/// Import a resource into a file table.
#[utoipa::path(post, path = "/api/console/terminal/files/import-resource", operation_id = "terminal.files.import_resource", request_body = ImportInput, responses((status = 200, body = ImportOutput), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn files_import_resource(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ImportInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.files.import_resource.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::Import(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
/// Download a file table record to the backend.
#[utoipa::path(post, path = "/api/console/terminal/files/download-file", operation_id = "terminal.files.download_file", request_body = DownloadInput, responses((status = 200, body = DownloadOutput), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn files_download_file(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<DownloadInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.files.download_file.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::Download(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
/// Start a backend command job.
#[utoipa::path(post, path = "/api/console/terminal/jobs/start", operation_id = "terminal.jobs.start", request_body = StartInput, responses((status = 200, body = JobSnapshot), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn jobs_start(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<StartInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.jobs.start.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::Start(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
/// Inspect a backend command job.
#[utoipa::path(post, path = "/api/console/terminal/jobs/get", operation_id = "terminal.jobs.get", request_body = JobInput, responses((status = 200, body = JobSnapshot), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn jobs_get(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<JobInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.jobs.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::Get(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
/// Read backend command job output.
#[utoipa::path(post, path = "/api/console/terminal/jobs/read", operation_id = "terminal.jobs.read", request_body = JobReadInput, responses((status = 200, body = ReadOutput), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn jobs_read(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<JobReadInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.jobs.read.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::Read(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
/// Cancel a backend command job.
#[utoipa::path(post, path = "/api/console/terminal/jobs/cancel", operation_id = "terminal.jobs.cancel", request_body = JobInput, responses((status = 200, body = JobSnapshot), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn jobs_cancel(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<JobInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let output = console_interface::invoke::<_, TerminalOutput>(
        Arc::clone(&state),
        "http.console.terminal.jobs.cancel.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        TerminalInput::Cancel(body),
    )
    .await?;
    Ok(Json(
        serde_json::to_value(output).map_err(anyhow::Error::from)?,
    ))
}
