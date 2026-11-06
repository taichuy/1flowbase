use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PathInput {
    pub path: String,
    pub timeout_ms: u64,
}
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ListInput {
    pub path: String,
    pub timeout_ms: u64,
    pub cursor: u64,
    pub limit: usize,
}
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TextInput {
    pub path: String,
    pub timeout_ms: u64,
    pub offset: u64,
    pub max_bytes: usize,
    pub expected_version: Option<String>,
}
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RangeInput {
    pub path: String,
    pub timeout_ms: u64,
    pub offset: u64,
    pub length: usize,
    pub expected_version: Option<String>,
}
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchInput {
    pub path: String,
    pub timeout_ms: u64,
    pub pattern: String,
    pub max_scan_bytes: u64,
    pub max_matches: usize,
    pub max_depth: usize,
    pub max_line_bytes: usize,
}
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MediaInput {
    pub path: String,
    pub timeout_ms: u64,
    pub max_file_bytes: u64,
}
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ResourceInput {
    pub path: String,
    pub timeout_ms: u64,
    pub max_file_bytes: u64,
    pub mime_type: Option<String>,
    #[serde(default)]
    pub link_only: bool,
}
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportInput {
    pub resource_uri: String,
    pub file_table_id: Uuid,
}
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DownloadInput {
    pub file_table_id: Uuid,
    pub record_id: Uuid,
    pub target_path: String,
    pub timeout_ms: u64,
    #[serde(default)]
    pub overwrite: bool,
    pub expected_sha256: Option<String>,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct FileMetadata {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size: u64,
    pub modified_at: Option<u64>,
    pub permissions: Option<u32>,
    pub version: String,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct ListOutput {
    pub path: String,
    pub entries: Vec<FileMetadata>,
    pub next_cursor: u64,
    pub eof: bool,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct TextOutput {
    pub path: String,
    pub offset: u64,
    pub text: String,
    pub next_offset: u64,
    pub eof: bool,
    pub version: String,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct RangeMetadata {
    pub path: String,
    pub offset: u64,
    pub byte_count: usize,
    pub next_offset: u64,
    pub eof: bool,
    pub version: String,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct RangeOutput {
    pub data: String,
    pub mime_type: String,
    pub metadata: RangeMetadata,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct MediaOutput {
    pub data: String,
    pub mime_type: String,
    pub metadata: FileMetadata,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct ResourceOutput {
    pub uri: String,
    pub name: String,
    pub data: Option<String>,
    pub mime_type: String,
    pub metadata: FileMetadata,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct SearchMatch {
    pub path: String,
    pub line_number: u64,
    pub byte_offset: u64,
    pub text: String,
}
#[derive(Debug, Default, Serialize, ToSchema)]
pub struct SearchOutput {
    pub matches: Vec<SearchMatch>,
    pub scanned_bytes: u64,
    pub truncated: bool,
    pub skipped_long_lines: u64,
    pub skipped_binary_files: u64,
    pub skipped_special_files: u64,
    pub depth_limited: bool,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct DownloadOutput {
    pub file_table_id: Uuid,
    pub record_id: Uuid,
    pub target_path: String,
    pub byte_count: u64,
    pub sha256: String,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct InfoOutput {
    pub os: String,
    pub shell: String,
    pub execution_location: String,
    pub capabilities: Vec<String>,
    pub pty: bool,
}
