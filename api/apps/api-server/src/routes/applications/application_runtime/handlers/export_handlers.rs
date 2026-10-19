#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/logs/runs/{run_id}/export",
    params(
        ("id" = String, Path, description = "Application id"),
        ("run_id" = String, Path, description = "Flow run id")
    ),
    responses(
        (status = 200, body = ApplicationRunTraceExportResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn export_application_run_trace_dump(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<axum::response::Response, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.trace-export.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_trace_exports::ApplicationRuntimeTraceExportsInput::ExportRun {
            application_id: id,
            run_id,
        },
    )
    .await?;
    let interface_trace_exports::ApplicationRuntimeTraceExportsOutput::Download(download) = output;
    download_response(download.content_type, &download.filename, download.body)
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/logs/runs/export",
    request_body = ApplicationRunSelectedExportBody,
    params(
        ("id" = String, Path, description = "Application id")
    ),
    responses(
        (status = 200, description = "Zip archive containing manifest.json and selected run JSON dumps", body = inline(crate::openapi::OpenApiBinaryBody), content_type = "application/zip"),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn export_application_runs_zip(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<ApplicationRunSelectedExportBody>,
) -> Result<axum::response::Response, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.trace-export.selected-runs.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state,
            headers,
        },
        interface_trace_exports::ApplicationRuntimeTraceExportsInput::ExportSelectedRuns {
            application_id: id,
            run_ids: body.run_ids,
        },
    )
    .await?;
    let interface_trace_exports::ApplicationRuntimeTraceExportsOutput::Download(download) = output;
    download_response(download.content_type, &download.filename, download.body)
}

fn download_response(
    content_type: &'static str,
    filename: &str,
    body: Vec<u8>,
) -> Result<axum::response::Response, ApiError> {
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, content_type)
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(axum::body::Body::from(body))
        .map_err(ApiError::from)
}

fn safe_filename_segment(value: &str) -> String {
    let mut segment = String::new();
    let mut last_was_separator = false;

    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if last_was_separator && !segment.is_empty() {
                segment.push('-');
            }
            segment.push(character.to_ascii_lowercase());
            last_was_separator = false;
        } else if matches!(character, '-' | '_' | '.' | ' ') || character.is_whitespace() {
            last_was_separator = true;
        }

        if segment.len() >= 64 {
            break;
        }
    }

    let segment = segment.trim_matches('-').to_string();
    if segment.is_empty() {
        "untitled".to_string()
    } else {
        segment
    }
}
const APPLICATION_RUN_TRACE_EXPORT_VERSION: i32 = 1;

struct ApplicationRunTraceExportDocument {
    title: String,
    started_at: OffsetDateTime,
    export_status: String,
    export_warning_count: usize,
    value: serde_json::Value,
}
