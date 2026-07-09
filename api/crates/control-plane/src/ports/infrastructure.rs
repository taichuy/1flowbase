use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

mod runtime_events;

pub use runtime_events::*;
pub const EPHEMERAL_INSPECTION_DEFAULT_LIMIT: usize = 50;
pub const EPHEMERAL_INSPECTION_MAX_LIMIT: usize = 200;
pub const EPHEMERAL_INSPECTION_DEFAULT_BYTE_LIMIT: usize = 64 * 1024;
pub const EPHEMERAL_INSPECTION_MAX_BYTE_LIMIT: usize = 256 * 1024;
pub const EPHEMERAL_VALUE_PREVIEW_BYTES: usize = 8 * 1024;
pub const EPHEMERAL_VALUE_FULL_BYTES: usize = 256 * 1024;
pub const EPHEMERAL_VALUE_MAX_BYTES: usize = 1024 * 1024;
pub const EPHEMERAL_PAYLOAD_MAX_BYTES: usize = 1024 * 1024;

pub struct CacheInspectionCapabilities {
    pub list_domains: bool,
    pub list_entries: bool,
    pub reveal_value: bool,
    pub clear_entry: bool,
    pub clear_domain: bool,
}

impl CacheInspectionCapabilities {
    pub const fn unsupported() -> Self {
        Self {
            list_domains: false,
            list_entries: false,
            reveal_value: false,
            clear_entry: false,
            clear_domain: false,
        }
    }

    pub const fn supported() -> Self {
        Self {
            list_domains: true,
            list_entries: true,
            reveal_value: true,
            clear_entry: true,
            clear_domain: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheDomainSnapshot {
    pub domain_code: String,
    pub entry_count: u64,
    pub total_value_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheEntrySnapshot {
    pub domain_code: String,
    pub key: String,
    pub value_size_bytes: u64,
    pub ttl_seconds: Option<i64>,
    pub created_at_unix: Option<i64>,
    pub expires_at_unix: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheEntryValueSnapshot {
    pub metadata: CacheEntrySnapshot,
    pub value: serde_json::Value,
}
pub struct EphemeralInspectionCapabilities {
    pub list_entries: bool,
    pub list_tree: bool,
    pub search_entries: bool,
    pub reveal_value: bool,
    pub default_page_size: u64,
    pub max_page_size: u64,
    pub default_byte_limit: u64,
    pub max_byte_limit: u64,
    pub default_preview_size_bytes: u64,
    pub max_full_value_size_bytes: u64,
    pub max_value_size_bytes: u64,
    pub max_payload_size_bytes: u64,
}

impl EphemeralInspectionCapabilities {
    pub const fn unsupported() -> Self {
        Self {
            list_entries: false,
            list_tree: false,
            search_entries: false,
            reveal_value: false,
            default_page_size: EPHEMERAL_INSPECTION_DEFAULT_LIMIT as u64,
            max_page_size: EPHEMERAL_INSPECTION_MAX_LIMIT as u64,
            default_byte_limit: EPHEMERAL_INSPECTION_DEFAULT_BYTE_LIMIT as u64,
            max_byte_limit: EPHEMERAL_INSPECTION_MAX_BYTE_LIMIT as u64,
            default_preview_size_bytes: EPHEMERAL_VALUE_PREVIEW_BYTES as u64,
            max_full_value_size_bytes: EPHEMERAL_VALUE_FULL_BYTES as u64,
            max_value_size_bytes: EPHEMERAL_VALUE_MAX_BYTES as u64,
            max_payload_size_bytes: EPHEMERAL_PAYLOAD_MAX_BYTES as u64,
        }
    }

    pub const fn supported() -> Self {
        Self {
            list_entries: true,
            list_tree: true,
            search_entries: true,
            reveal_value: true,
            default_page_size: EPHEMERAL_INSPECTION_DEFAULT_LIMIT as u64,
            max_page_size: EPHEMERAL_INSPECTION_MAX_LIMIT as u64,
            default_byte_limit: EPHEMERAL_INSPECTION_DEFAULT_BYTE_LIMIT as u64,
            max_byte_limit: EPHEMERAL_INSPECTION_MAX_BYTE_LIMIT as u64,
            default_preview_size_bytes: EPHEMERAL_VALUE_PREVIEW_BYTES as u64,
            max_full_value_size_bytes: EPHEMERAL_VALUE_FULL_BYTES as u64,
            max_value_size_bytes: EPHEMERAL_VALUE_MAX_BYTES as u64,
            max_payload_size_bytes: EPHEMERAL_PAYLOAD_MAX_BYTES as u64,
        }
    }

    pub const fn metadata_only() -> Self {
        Self {
            list_entries: true,
            list_tree: true,
            search_entries: true,
            reveal_value: false,
            default_page_size: EPHEMERAL_INSPECTION_DEFAULT_LIMIT as u64,
            max_page_size: EPHEMERAL_INSPECTION_MAX_LIMIT as u64,
            default_byte_limit: EPHEMERAL_INSPECTION_DEFAULT_BYTE_LIMIT as u64,
            max_byte_limit: EPHEMERAL_INSPECTION_MAX_BYTE_LIMIT as u64,
            default_preview_size_bytes: EPHEMERAL_VALUE_PREVIEW_BYTES as u64,
            max_full_value_size_bytes: EPHEMERAL_VALUE_FULL_BYTES as u64,
            max_value_size_bytes: EPHEMERAL_VALUE_MAX_BYTES as u64,
            max_payload_size_bytes: EPHEMERAL_PAYLOAD_MAX_BYTES as u64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EphemeralInspectionPageRequest {
    pub inspection_path: Vec<String>,
    pub cursor: Option<String>,
    pub limit: usize,
    pub byte_limit: usize,
}

impl EphemeralInspectionPageRequest {
    pub fn new(
        inspection_path: Vec<String>,
        cursor: Option<String>,
        limit: Option<usize>,
        byte_limit: Option<usize>,
    ) -> Self {
        Self {
            inspection_path,
            cursor,
            limit: limit
                .unwrap_or(EPHEMERAL_INSPECTION_DEFAULT_LIMIT)
                .clamp(1, EPHEMERAL_INSPECTION_MAX_LIMIT),
            byte_limit: byte_limit
                .unwrap_or(EPHEMERAL_INSPECTION_DEFAULT_BYTE_LIMIT)
                .clamp(1, EPHEMERAL_INSPECTION_MAX_BYTE_LIMIT),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EphemeralInspectionSummarySnapshot {
    pub entry_count: u64,
    pub sensitive_entry_count: u64,
    pub total_value_size_bytes: u64,
}

impl EphemeralInspectionSummarySnapshot {
    pub const fn empty() -> Self {
        Self {
            entry_count: 0,
            sensitive_entry_count: 0,
            total_value_size_bytes: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EphemeralValueRevealMode {
    Metadata,
    #[default]
    Preview,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EphemeralEntryValueState {
    Hidden,
    Available,
    Preview,
    ValueTooLarge,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EphemeralInspectionTreeNodeSnapshot {
    pub node_ref: String,
    pub label: String,
    pub inspection_path: Vec<String>,
    pub depth: u64,
    pub entry_count: u64,
    pub sensitive_entry_count: u64,
    pub total_value_size_bytes: u64,
    pub has_children: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EphemeralInspectionTreePage {
    pub inspection_path: Vec<String>,
    pub nodes: Vec<EphemeralInspectionTreeNodeSnapshot>,
    pub next_cursor: Option<String>,
    pub limit: u64,
    pub byte_limit: u64,
    pub emitted_bytes: u64,
    pub truncated_by_byte_limit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EphemeralInspectionEntryPage {
    pub inspection_path: Vec<String>,
    pub entries: Vec<EphemeralEntrySnapshot>,
    pub next_cursor: Option<String>,
    pub limit: u64,
    pub byte_limit: u64,
    pub emitted_bytes: u64,
    pub truncated_by_byte_limit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EphemeralEntrySnapshot {
    pub contract_code: String,
    pub group_code: Option<String>,
    pub entry_ref: String,
    pub key: String,
    pub inspection_path: Vec<String>,
    pub entry_kind: String,
    pub status: String,
    pub owner: Option<String>,
    pub value_size_bytes: u64,
    pub metadata_size_bytes: u64,
    pub ttl_seconds: Option<i64>,
    pub created_at_unix: Option<i64>,
    pub expires_at_unix: Option<i64>,
    pub sensitive: bool,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EphemeralEntryValueSnapshot {
    pub metadata: EphemeralEntrySnapshot,
    pub reveal_mode: EphemeralValueRevealMode,
    pub value_state: EphemeralEntryValueState,
    pub value: Option<serde_json::Value>,
    pub value_preview: Option<String>,
    pub preview_size_bytes: u64,
    pub full_value_size_bytes: u64,
}

impl EphemeralEntryValueSnapshot {
    pub fn from_value(
        metadata: EphemeralEntrySnapshot,
        value: serde_json::Value,
        reveal_mode: EphemeralValueRevealMode,
    ) -> Self {
        let serialized = serde_json::to_string(&value).unwrap_or_default();
        let full_value_size_bytes = serialized.len() as u64;
        match reveal_mode {
            EphemeralValueRevealMode::Metadata => Self {
                metadata,
                reveal_mode,
                value_state: EphemeralEntryValueState::Hidden,
                value: None,
                value_preview: None,
                preview_size_bytes: 0,
                full_value_size_bytes,
            },
            EphemeralValueRevealMode::Preview
                if serialized.len() <= EPHEMERAL_VALUE_PREVIEW_BYTES =>
            {
                Self {
                    metadata,
                    reveal_mode,
                    value_state: EphemeralEntryValueState::Available,
                    value: Some(value),
                    value_preview: None,
                    preview_size_bytes: full_value_size_bytes,
                    full_value_size_bytes,
                }
            }
            EphemeralValueRevealMode::Preview => {
                let preview = truncate_json_preview(&serialized, EPHEMERAL_VALUE_PREVIEW_BYTES);
                Self {
                    metadata,
                    reveal_mode,
                    value_state: EphemeralEntryValueState::Preview,
                    value: None,
                    preview_size_bytes: preview.len() as u64,
                    value_preview: Some(preview),
                    full_value_size_bytes,
                }
            }
            EphemeralValueRevealMode::Full if serialized.len() <= EPHEMERAL_VALUE_FULL_BYTES => {
                Self {
                    metadata,
                    reveal_mode,
                    value_state: EphemeralEntryValueState::Available,
                    value: Some(value),
                    value_preview: None,
                    preview_size_bytes: full_value_size_bytes,
                    full_value_size_bytes,
                }
            }
            EphemeralValueRevealMode::Full => Self {
                metadata,
                reveal_mode,
                value_state: EphemeralEntryValueState::ValueTooLarge,
                value: None,
                value_preview: None,
                preview_size_bytes: 0,
                full_value_size_bytes,
            },
        }
    }
}

pub fn ephemeral_metadata_size_bytes(metadata: &serde_json::Value) -> u64 {
    serde_json::to_vec(metadata)
        .map(|bytes| bytes.len() as u64)
        .unwrap_or(0)
}

pub fn ensure_ephemeral_value_size(value: &serde_json::Value) -> anyhow::Result<()> {
    ensure_ephemeral_json_size(
        value,
        EPHEMERAL_VALUE_MAX_BYTES,
        "ephemeral_value_too_large",
    )
}

pub fn ensure_ephemeral_payload_size(value: &serde_json::Value) -> anyhow::Result<()> {
    ensure_ephemeral_json_size(
        value,
        EPHEMERAL_PAYLOAD_MAX_BYTES,
        "ephemeral_payload_too_large",
    )
}

pub fn summarize_ephemeral_entries(
    entries: &[EphemeralEntrySnapshot],
) -> EphemeralInspectionSummarySnapshot {
    EphemeralInspectionSummarySnapshot {
        entry_count: entries.len() as u64,
        sensitive_entry_count: entries.iter().filter(|entry| entry.sensitive).count() as u64,
        total_value_size_bytes: entries
            .iter()
            .map(|entry| entry.value_size_bytes)
            .sum::<u64>(),
    }
}

pub fn paginate_ephemeral_entries(
    mut entries: Vec<EphemeralEntrySnapshot>,
    request: EphemeralInspectionPageRequest,
) -> EphemeralInspectionEntryPage {
    entries.sort_by_key(entry_cursor_token);
    let mut page_entries = Vec::new();
    let mut emitted_bytes = 0usize;
    let mut truncated_by_byte_limit = false;
    let filtered_entries = entries
        .into_iter()
        .filter(|entry| path_has_prefix(&entry.inspection_path, &request.inspection_path))
        .filter(|entry| {
            request
                .cursor
                .as_ref()
                .is_none_or(|cursor| entry_cursor_token(entry).as_str() > cursor.as_str())
        })
        .collect::<Vec<_>>();

    for entry in filtered_entries.iter().cloned() {
        let item_size = serialized_size_bytes(&entry);
        if !page_entries.is_empty()
            && (page_entries.len() >= request.limit
                || emitted_bytes.saturating_add(item_size) > request.byte_limit)
        {
            truncated_by_byte_limit = emitted_bytes.saturating_add(item_size) > request.byte_limit;
            break;
        }
        emitted_bytes = emitted_bytes.saturating_add(item_size);
        page_entries.push(entry);
        if page_entries.len() >= request.limit {
            break;
        }
    }

    let next_cursor = page_entries
        .last()
        .filter(|last| {
            let last_cursor = entry_cursor_token(last);
            filtered_entries
                .iter()
                .any(|entry| entry_cursor_token(entry) > last_cursor)
        })
        .map(entry_cursor_token);
    EphemeralInspectionEntryPage {
        inspection_path: request.inspection_path,
        entries: page_entries,
        next_cursor,
        limit: request.limit as u64,
        byte_limit: request.byte_limit as u64,
        emitted_bytes: emitted_bytes as u64,
        truncated_by_byte_limit,
    }
}

pub fn paginate_ephemeral_tree(
    entries: Vec<EphemeralEntrySnapshot>,
    request: EphemeralInspectionPageRequest,
) -> EphemeralInspectionTreePage {
    let mut nodes = BTreeMap::<String, EphemeralInspectionTreeNodeSnapshot>::new();
    for entry in entries {
        if !path_has_prefix(&entry.inspection_path, &request.inspection_path)
            || entry.inspection_path.len() <= request.inspection_path.len()
        {
            continue;
        }
        let child_depth = request.inspection_path.len();
        let label = entry.inspection_path[child_depth].clone();
        let inspection_path = entry.inspection_path[..=child_depth].to_vec();
        let node_ref = inspection_path.join("/");
        let node = nodes
            .entry(node_ref.clone())
            .or_insert(EphemeralInspectionTreeNodeSnapshot {
                node_ref,
                label,
                inspection_path,
                depth: (child_depth + 1) as u64,
                entry_count: 0,
                sensitive_entry_count: 0,
                total_value_size_bytes: 0,
                has_children: false,
            });
        node.entry_count += 1;
        node.sensitive_entry_count += u64::from(entry.sensitive);
        node.total_value_size_bytes += entry.value_size_bytes;
        node.has_children |= entry.inspection_path.len() > child_depth + 1;
    }

    let mut page_nodes = Vec::new();
    let mut emitted_bytes = 0usize;
    let mut truncated_by_byte_limit = false;
    let mut filtered_nodes = nodes
        .into_values()
        .filter(|node| {
            request
                .cursor
                .as_ref()
                .is_none_or(|cursor| tree_cursor_token(node).as_str() > cursor.as_str())
        })
        .collect::<Vec<_>>();
    filtered_nodes.sort_by_key(tree_cursor_token);
    for node in filtered_nodes.iter().cloned() {
        let item_size = serialized_size_bytes(&node);
        if !page_nodes.is_empty()
            && (page_nodes.len() >= request.limit
                || emitted_bytes.saturating_add(item_size) > request.byte_limit)
        {
            truncated_by_byte_limit = emitted_bytes.saturating_add(item_size) > request.byte_limit;
            break;
        }
        emitted_bytes = emitted_bytes.saturating_add(item_size);
        page_nodes.push(node);
        if page_nodes.len() >= request.limit {
            break;
        }
    }

    let next_cursor = page_nodes
        .last()
        .filter(|last| {
            let last_cursor = tree_cursor_token(last);
            filtered_nodes
                .iter()
                .any(|node| tree_cursor_token(node) > last_cursor)
        })
        .map(tree_cursor_token);
    EphemeralInspectionTreePage {
        inspection_path: request.inspection_path,
        nodes: page_nodes,
        next_cursor,
        limit: request.limit as u64,
        byte_limit: request.byte_limit as u64,
        emitted_bytes: emitted_bytes as u64,
        truncated_by_byte_limit,
    }
}

pub fn search_ephemeral_entries(
    entries: Vec<EphemeralEntrySnapshot>,
    query: &str,
    request: EphemeralInspectionPageRequest,
) -> EphemeralInspectionEntryPage {
    let normalized_query = query.trim().to_lowercase();
    if normalized_query.is_empty() {
        return paginate_ephemeral_entries(entries, request);
    }
    let matched_entries = entries
        .into_iter()
        .filter(|entry| ephemeral_entry_matches_query(entry, &normalized_query))
        .collect::<Vec<_>>();
    paginate_ephemeral_entries(matched_entries, request)
}

fn ephemeral_entry_matches_query(entry: &EphemeralEntrySnapshot, query: &str) -> bool {
    let mut searchable = [
        entry.entry_ref.as_str(),
        entry.key.as_str(),
        entry.contract_code.as_str(),
        entry.entry_kind.as_str(),
        entry.status.as_str(),
    ]
    .join(" ");
    if let Some(group_code) = &entry.group_code {
        searchable.push(' ');
        searchable.push_str(group_code);
    }
    if let Some(owner) = &entry.owner {
        searchable.push(' ');
        searchable.push_str(owner);
    }
    searchable.push(' ');
    searchable.push_str(&entry.inspection_path.join("/"));
    searchable.push(' ');
    searchable.push_str(&entry.metadata.to_string());
    searchable.to_lowercase().contains(query)
}

fn path_has_prefix(path: &[String], prefix: &[String]) -> bool {
    prefix.len() <= path.len()
        && prefix
            .iter()
            .zip(path.iter())
            .all(|(prefix, segment)| prefix == segment)
}

fn serialized_size_bytes<T: Serialize>(value: &T) -> usize {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

fn entry_cursor_token(entry: &EphemeralEntrySnapshot) -> String {
    format!(
        "{}\u{1f}{}",
        path_cursor_token(&entry.inspection_path),
        normalized_cursor_segment(&entry.entry_ref)
    )
}

fn tree_cursor_token(node: &EphemeralInspectionTreeNodeSnapshot) -> String {
    path_cursor_token(&node.inspection_path)
}

fn path_cursor_token(path: &[String]) -> String {
    path.iter()
        .map(|segment| normalized_cursor_segment(segment))
        .collect::<Vec<_>>()
        .join("\u{1f}")
}

fn normalized_cursor_segment(segment: &str) -> String {
    segment
        .parse::<u128>()
        .map(|value| format!("n{value:039}"))
        .unwrap_or_else(|_| format!("s{segment}"))
}

fn truncate_json_preview(value: &str, byte_limit: usize) -> String {
    let mut end = byte_limit.min(value.len());
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    value[..end].to_string()
}

fn ensure_ephemeral_json_size(
    value: &serde_json::Value,
    max_bytes: usize,
    error_code: &'static str,
) -> anyhow::Result<()> {
    let size = serde_json::to_vec(value).map(|bytes| bytes.len())?;
    anyhow::ensure!(size <= max_bytes, error_code);
    Ok(())
}

#[async_trait]
pub trait CacheStore: Send + Sync {
    async fn get_json(&self, key: &str) -> anyhow::Result<Option<serde_json::Value>>;

    async fn set_json(
        &self,
        key: &str,
        value: serde_json::Value,
        ttl: Option<time::Duration>,
    ) -> anyhow::Result<()>;

    async fn set_if_absent_json(
        &self,
        key: &str,
        value: serde_json::Value,
        ttl: Option<time::Duration>,
    ) -> anyhow::Result<bool>;

    async fn increment_counter(
        &self,
        key: &str,
        amount: i64,
        ttl: Option<time::Duration>,
    ) -> anyhow::Result<i64>;

    async fn delete(&self, key: &str) -> anyhow::Result<()>;

    async fn touch(&self, key: &str, ttl: time::Duration) -> anyhow::Result<bool>;

    fn inspection_capabilities(&self) -> CacheInspectionCapabilities {
        CacheInspectionCapabilities::unsupported()
    }

    async fn list_cache_domains(&self) -> anyhow::Result<Vec<CacheDomainSnapshot>> {
        Ok(Vec::new())
    }

    async fn list_cache_entries(
        &self,
        _domain_code: &str,
    ) -> anyhow::Result<Vec<CacheEntrySnapshot>> {
        Ok(Vec::new())
    }

    async fn reveal_cache_entry(
        &self,
        _domain_code: &str,
        _key: &str,
    ) -> anyhow::Result<Option<CacheEntryValueSnapshot>> {
        Ok(None)
    }

    async fn clear_cache_entry(&self, _domain_code: &str, _key: &str) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn clear_cache_domain(&self, _domain_code: &str) -> anyhow::Result<u64> {
        Ok(0)
    }

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::unsupported()
    }

    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        Ok(Vec::new())
    }

    async fn summarize_ephemeral_entries(
        &self,
    ) -> anyhow::Result<EphemeralInspectionSummarySnapshot> {
        Ok(summarize_ephemeral_entries(
            &self.list_ephemeral_entries().await?,
        ))
    }

    async fn list_ephemeral_tree(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionTreePage> {
        Ok(paginate_ephemeral_tree(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn list_ephemeral_entry_page(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(paginate_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn search_ephemeral_entry_page(
        &self,
        query: &str,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(search_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            query,
            request,
        ))
    }

    async fn reveal_ephemeral_entry(
        &self,
        _entry_ref: &str,
        _reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        Ok(None)
    }
}

#[async_trait]
pub trait DistributedLock: Send + Sync {
    async fn acquire(&self, key: &str, owner: &str, ttl: time::Duration) -> anyhow::Result<bool>;

    async fn renew(&self, key: &str, owner: &str, ttl: time::Duration) -> anyhow::Result<bool>;

    async fn release(&self, key: &str, owner: &str) -> anyhow::Result<bool>;

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::unsupported()
    }

    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        Ok(Vec::new())
    }

    async fn summarize_ephemeral_entries(
        &self,
    ) -> anyhow::Result<EphemeralInspectionSummarySnapshot> {
        Ok(summarize_ephemeral_entries(
            &self.list_ephemeral_entries().await?,
        ))
    }

    async fn list_ephemeral_tree(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionTreePage> {
        Ok(paginate_ephemeral_tree(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn list_ephemeral_entry_page(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(paginate_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn search_ephemeral_entry_page(
        &self,
        query: &str,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(search_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            query,
            request,
        ))
    }

    async fn reveal_ephemeral_entry(
        &self,
        _entry_ref: &str,
        _reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        Ok(None)
    }
}

#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, topic: &str, payload: serde_json::Value) -> anyhow::Result<()>;

    async fn poll(&self, topic: &str) -> anyhow::Result<Option<serde_json::Value>>;

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::unsupported()
    }

    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        Ok(Vec::new())
    }

    async fn summarize_ephemeral_entries(
        &self,
    ) -> anyhow::Result<EphemeralInspectionSummarySnapshot> {
        Ok(summarize_ephemeral_entries(
            &self.list_ephemeral_entries().await?,
        ))
    }

    async fn list_ephemeral_tree(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionTreePage> {
        Ok(paginate_ephemeral_tree(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn list_ephemeral_entry_page(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(paginate_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn search_ephemeral_entry_page(
        &self,
        query: &str,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(search_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            query,
            request,
        ))
    }

    async fn reveal_ephemeral_entry(
        &self,
        _entry_ref: &str,
        _reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        Ok(None)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaimedTask {
    pub task_id: String,
    pub payload: serde_json::Value,
    pub claimed_by: String,
    pub idempotency_key: Option<String>,
    pub claim_expires_at_unix: i64,
}

#[async_trait]
pub trait TaskQueue: Send + Sync {
    async fn enqueue(
        &self,
        queue: &str,
        payload: serde_json::Value,
        idempotency_key: Option<&str>,
    ) -> anyhow::Result<String>;

    async fn claim(
        &self,
        queue: &str,
        worker: &str,
        visibility_timeout: time::Duration,
    ) -> anyhow::Result<Option<ClaimedTask>>;

    async fn ack(&self, queue: &str, task_id: &str, worker: &str) -> anyhow::Result<bool>;

    async fn fail(
        &self,
        queue: &str,
        task_id: &str,
        worker: &str,
        reason: &str,
    ) -> anyhow::Result<bool>;

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::unsupported()
    }

    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        Ok(Vec::new())
    }

    async fn summarize_ephemeral_entries(
        &self,
    ) -> anyhow::Result<EphemeralInspectionSummarySnapshot> {
        Ok(summarize_ephemeral_entries(
            &self.list_ephemeral_entries().await?,
        ))
    }

    async fn list_ephemeral_tree(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionTreePage> {
        Ok(paginate_ephemeral_tree(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn list_ephemeral_entry_page(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(paginate_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn search_ephemeral_entry_page(
        &self,
        query: &str,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(search_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            query,
            request,
        ))
    }

    async fn reveal_ephemeral_entry(
        &self,
        _entry_ref: &str,
        _reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        Ok(None)
    }
}
pub struct RateLimitDecision {
    pub allowed: bool,
    pub remaining: u64,
    pub reset_after_ms: u64,
}

#[async_trait]
pub trait RateLimitStore: Send + Sync {
    async fn consume(
        &self,
        key: &str,
        limit: u64,
        window: time::Duration,
    ) -> anyhow::Result<RateLimitDecision>;

    async fn reset(&self, key: &str) -> anyhow::Result<()>;

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::unsupported()
    }

    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        Ok(Vec::new())
    }

    async fn summarize_ephemeral_entries(
        &self,
    ) -> anyhow::Result<EphemeralInspectionSummarySnapshot> {
        Ok(summarize_ephemeral_entries(
            &self.list_ephemeral_entries().await?,
        ))
    }

    async fn list_ephemeral_tree(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionTreePage> {
        Ok(paginate_ephemeral_tree(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn list_ephemeral_entry_page(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(paginate_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn search_ephemeral_entry_page(
        &self,
        query: &str,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(search_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            query,
            request,
        ))
    }

    async fn reveal_ephemeral_entry(
        &self,
        _entry_ref: &str,
        _reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        Ok(None)
    }
}
