use super::*;
use control_plane::frontstage as source;

#[derive(Debug, Deserialize, IntoParams)]
#[serde(deny_unknown_fields)]
pub struct SearchFrontstageBlockCodeQuery {
    pub query: String,
    #[serde(default = "default_source_start_line")]
    pub start_line: u32,
    #[serde(default = "default_source_start_column")]
    pub start_column: u32,
    #[serde(default = "default_code_search_limit")]
    pub limit: u32,
    pub expected_source_revision: Option<String>,
}
fn default_code_search_limit() -> u32 {
    20
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FrontstageSourceTextEditBody {
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplaceFrontstageBlockCodeBody {
    pub expected_source_revision: String,
    pub edits: Vec<FrontstageSourceTextEditBody>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SourcePositionResponse {
    pub line: u32,
    pub column: u32,
}

impl From<source::SourcePosition> for SourcePositionResponse {
    fn from(value: source::SourcePosition) -> Self {
        Self {
            line: value.line,
            column: value.column,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageSourceMatchResponse {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub context: String,
    pub context_start_line: u32,
    pub context_start_column: u32,
    pub context_truncated: bool,
}

impl From<source::FrontstageSourceMatch> for FrontstageSourceMatchResponse {
    fn from(value: source::FrontstageSourceMatch) -> Self {
        Self {
            start_line: value.start_line,
            start_column: value.start_column,
            end_line: value.end_line,
            end_column: value.end_column,
            context: value.context,
            context_start_line: value.context_start_line,
            context_start_column: value.context_start_column,
            context_truncated: value.context_truncated,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageCodeSearchResponse {
    pub block_id: String,
    pub page_id: String,
    pub source_revision: String,
    pub matches: Vec<FrontstageSourceMatchResponse>,
    pub truncated: bool,
    pub next_line: Option<u32>,
    pub next_column: Option<u32>,
}

impl From<source::FrontstageCodeSearch> for FrontstageCodeSearchResponse {
    fn from(value: source::FrontstageCodeSearch) -> Self {
        Self {
            block_id: value.block_id,
            page_id: value.page_id.to_string(),
            source_revision: value.source_revision,
            matches: value.matches.into_iter().map(Into::into).collect(),
            truncated: value.truncated,
            next_line: value.next_line,
            next_column: value.next_column,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageSourceChangeResponse {
    pub edit_index: usize,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub old_text: String,
    pub new_text: String,
    pub truncated: bool,
}

impl From<source::FrontstageSourceChange> for FrontstageSourceChangeResponse {
    fn from(value: source::FrontstageSourceChange) -> Self {
        Self {
            edit_index: value.edit_index,
            start_line: value.start_line,
            start_column: value.start_column,
            end_line: value.end_line,
            end_column: value.end_column,
            old_text: value.old_text,
            new_text: value.new_text,
            truncated: value.truncated,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageCodeEditReceiptResponse {
    pub block_id: String,
    pub page_id: String,
    pub source_revision: String,
    pub applied_edits: usize,
    pub changes: Vec<FrontstageSourceChangeResponse>,
    pub diff_truncated: bool,
}

impl From<source::FrontstageCodeEditReceipt> for FrontstageCodeEditReceiptResponse {
    fn from(value: source::FrontstageCodeEditReceipt) -> Self {
        Self {
            block_id: value.block_id,
            page_id: value.page_id.to_string(),
            source_revision: value.source_revision,
            applied_edits: value.applied_edits,
            changes: value.changes.into_iter().map(Into::into).collect(),
            diff_truncated: value.diff_truncated,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageSourceEditErrorDetails {
    pub edit_index: Option<usize>,
    pub current_source_revision: Option<String>,
    pub candidates: Vec<SourcePositionResponse>,
    pub candidates_truncated: bool,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageSourceEditErrorResponse {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub details: Option<FrontstageSourceEditErrorDetails>,
}

pub(crate) fn source_edit_error_response(
    error: &source::FrontstageSourceEditError,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let status = if error.code == "frontstage_block_source_revision" {
        StatusCode::CONFLICT
    } else {
        StatusCode::BAD_REQUEST
    };
    (
        status,
        Json(FrontstageSourceEditErrorResponse {
            status: status.as_u16(),
            code: error.code.to_owned(),
            message: error.to_string(),
            details: Some(FrontstageSourceEditErrorDetails {
                edit_index: error.edit_index,
                current_source_revision: error.current_source_revision.clone(),
                candidates: error.candidates.iter().cloned().map(Into::into).collect(),
                candidates_truncated: error.candidates_truncated,
            }),
        }),
    )
        .into_response()
}

#[utoipa::path(get, path = "/api/console/frontstage/pages/{page_id}/blocks/{block_id}/code/search",
    params(SearchFrontstageBlockCodeQuery),
    summary = "Search a Frontstage block source",
    description = "Finds literal text in one visible block. Returns at most 100 matches (default 20), each with up to 320 Unicode scalars of context and 1-based Unicode scalar coordinates. Query is nonempty and at most 8000 bytes. Continue at next_line/next_column with expected_source_revision to reject changed snapshots.",
    responses((status = 200, body = FrontstageCodeSearchResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 409, body = FrontstageSourceEditErrorResponse)))]
pub async fn search_frontstage_block_code(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((page_id, block_id)): Path<(String, String)>,
    Query(query): Query<SearchFrontstageBlockCodeQuery>,
) -> Result<Json<ApiSuccess<FrontstageCodeSearchResponse>>, ApiError> {
    let interface::FrontstageBlocksOutput::CodeSearch(value) = invoke_blocks(
        state,
        headers,
        "http.console.frontstage.blocks.code-search.get.v1",
        interface::FrontstageBlocksInput::SearchCode(page_id, block_id, query),
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(post, path = "/api/console/frontstage/pages/{page_id}/blocks/{block_id}/code/replace",
    request_body = ReplaceFrontstageBlockCodeBody,
    summary = "Replace exact Frontstage source fragments",
    description = "Atomically replaces 1 to 100 non-overlapping uniquely matching old_text fragments from one source revision. Empty new_text deletes; preserving an anchor and adding text inserts. Empty old_text, absent or ambiguous matches and stale revisions are rejected. Edits total at most 1000000 UTF-8 bytes. Returns original half-open coordinates and at most 8000 Unicode scalars of before/after text across all changes, with truncation flags; never returns full source by default.",
    responses((status = 200, body = FrontstageCodeEditReceiptResponse), (status = 400, body = FrontstageSourceEditErrorResponse), (status = 409, body = FrontstageSourceEditErrorResponse)))]
pub async fn replace_frontstage_block_code(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((page_id, block_id)): Path<(String, String)>,
    Json(body): Json<ReplaceFrontstageBlockCodeBody>,
) -> Result<Json<ApiSuccess<FrontstageCodeEditReceiptResponse>>, ApiError> {
    let interface::FrontstageBlocksOutput::EditedCode(value) = invoke_blocks(
        state,
        headers,
        "http.console.frontstage.blocks.code-replace.post.v1",
        interface::FrontstageBlocksInput::ReplaceCode(page_id, block_id, body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}
