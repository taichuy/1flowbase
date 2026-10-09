//! Typed, source-neutral log queries; the legacy GUI readers keep their contracts.
use super::*;
use control_plane::ports::{
    ApplicationLogRecordsPage, LogQueryField, RecordClientTrajectoryQueryPage,
};
pub(super) mod schemas;
use schemas::{LogRecordsPageSchema, QueryFieldsSchema, TrajectoryQueryPageSchema};

#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LogRecordsQuery {
    /// Omit for all applications readable in the current workspace. An empty array selects none.
    pub application_ids: Option<Vec<Uuid>>,
    /// Existing resource grammar: $and/$or and field comparisons. See query-fields.
    pub filter: Option<serde_json::Value>,
    /// A declared datetime field, default started_at. Equal values are ordered by record UUID.
    pub sort_field: Option<String>,
    pub descending: Option<bool>,
    /// Reuse only with the same scope, application selection, filter and sort.
    pub cursor: Option<String>,
    /// Page size; positive values only. This limits output, not the rows searched.
    pub limit: Option<i64>,
    /// Case-insensitive SQL contains across task title, user_input and final_output snapshots.
    /// Existing $includes semantics apply: % and _ are SQL pattern wildcards.
    pub keyword: Option<String>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TrajectoryQuery {
    /// Existing resource grammar, limited to the fields declared by query-fields.
    pub filter: Option<serde_json::Value>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    /// Search restored semantic section bodies, not only previews. Raw archive search is excluded.
    pub keyword: Option<String>,
    /// overview, parameters or result; default result when keyword is present.
    pub search_sections: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct QueryFields {
    pub record_fields: Vec<LogQueryField>,
    pub trajectory_fields: Vec<LogQueryField>,
    pub logical_operators: Vec<String>,
    pub record_keyword_fields: Vec<String>,
    pub trajectory_search_sections: Vec<String>,
    pub record_contains_semantics: String,
    pub trajectory_keyword_semantics: String,
    pub pagination: String,
}
impl Default for QueryFields {
    fn default() -> Self {
        Self {
            record_fields: control_plane::ports::application_log_query_fields(),
            trajectory_fields: control_plane::ports::record_trajectory_query_fields(),
            logical_operators: vec!["$and".into(), "$or".into()],
            record_keyword_fields: vec!["title".into(), "user_input".into(), "final_output".into()],
            trajectory_search_sections: vec!["overview".into(), "parameters".into(), "result".into()],
            record_contains_semantics: "Case-insensitive SQL ILIKE on full task snapshots; % and _ are pattern wildcards. No raw protocol archive search.".into(),
            trajectory_keyword_semantics: "Case-insensitive substring on restored selected semantic section bodies; matches identify step_id and section. Preview fields filter summaries only.".into(),
            pagination: "Opaque keyset cursor bound to query and authorized selection; reuse unchanged query. next_cursor=null ends the query. limit bounds output, not scan cost; no exact total count.".into(),
        }
    }
}

/// Query Native and imported log records together.
/// Applies current-workspace and application visibility before typed field predicates. Returns stable cursor pages without loading trajectory bodies or counting the entire matching set.
#[utoipa::path(post, path="/api/console/applications/logs/records/query", request_body=LogRecordsQuery, responses((status=200, body=LogRecordsPageSchema)))]
pub async fn query_log_records(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(query): Json<LogRecordsQuery>,
) -> Result<Json<ApiSuccess<ApplicationLogRecordsPage>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.records.query.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::QueryRecords { query },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::Records(page) = output else {
        unreachable!("records query binding output")
    };
    Ok(Json(ApiSuccess::new(page)))
}

/// Discover source-neutral log and trajectory query fields.
/// Returns typed field names, allowed comparison operators, sortable fields, explicit keyword scope and cursor semantics. Does not register composed trajectory as a table.
#[utoipa::path(get, path="/api/console/applications/logs/query-fields", responses((status=200, body=QueryFieldsSchema)))]
pub async fn get_query_fields(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<QueryFields>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.records.query-fields.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::QueryFields,
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::QueryFields(fields) = output else {
        unreachable!("query fields binding output")
    };
    Ok(Json(ApiSuccess::new(fields)))
}

/// Filter client trajectory and locate semantic section keyword hits.
/// Searches only this authorized record. Restores selected semantic sections on demand, returns real step/section locators, and preserves the legacy list and raw section readers.
#[utoipa::path(post, path="/api/console/applications/{id}/logs/records/{record_id}/client-trajectory/query", params(("id"=Uuid,Path),("record_id"=Uuid,Path)), request_body=TrajectoryQuery, responses((status=200, body=TrajectoryQueryPageSchema)))]
pub async fn query_record_trajectory(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, record_id)): Path<(Uuid, Uuid)>,
    Json(query): Json<TrajectoryQuery>,
) -> Result<Json<ApiSuccess<RecordClientTrajectoryQueryPage>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.runtime.record.client-trajectory.query.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_runtime_reads::ApplicationRuntimeReadsInput::QueryRecordTrajectory {
            application_id,
            record_id,
            query,
        },
    )
    .await?;
    let interface_runtime_reads::ApplicationRuntimeReadsOutput::RecordTrajectoryQuery(page) =
        output
    else {
        unreachable!("record trajectory query binding output")
    };
    Ok(Json(ApiSuccess::new(page)))
}

pub(super) fn records_filter(
    query: &LogRecordsQuery,
) -> Result<domain::ResourceFilterExpr, ControlPlaneError> {
    let base = control_plane::resource_crud::parse_resource_filter_expr(
        query.filter.as_ref().unwrap_or(&serde_json::json!({})),
    )?;
    let Some(keyword) = query.keyword.as_ref().filter(|value| !value.is_empty()) else {
        return Ok(base);
    };
    let keyword = control_plane::resource_crud::parse_resource_filter_expr(
        &serde_json::json!({"$or":[{"title":{"$includes":keyword}},{"user_input":{"$includes":keyword}},{"final_output":{"$includes":keyword}}]}),
    )?;
    Ok(domain::ResourceFilterExpr::all(vec![base, keyword]))
}
