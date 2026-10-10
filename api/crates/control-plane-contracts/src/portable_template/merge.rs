//! Durable per-resource ownership and write intent. No editor snapshot is an applied baseline.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TemplateResourceKey {
    pub kind: String,
    pub source_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateBaselineScope {
    pub workspace_id: Uuid,
    pub template_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateWriteIntent {
    pub operation_id: Uuid,
    pub target_id: String,
    /// None means the owner must atomically prove absence before creating.
    pub expected_fingerprint: Option<String>,
    pub desired_fingerprint: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateResourceBaseline {
    pub key: TemplateResourceKey,
    pub target_id: String,
    pub generation: i64,
    pub applied_fingerprint: Option<String>,
    pub pending: Option<TemplateWriteIntent>,
    /// Written in the SAME transaction as the guarded resource mutation.
    /// A retry may finalize this receipt, but may never infer it from current content.
    pub committed_operation_id: Option<Uuid>,
    pub committed_fingerprint: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateMergeDecision {
    Initialize,
    Update,
    Unchanged,
    SkipUnknownBaseline,
    SkipUserModified,
    SkipUserDeleted,
    /// An interrupted owner write has no atomic receipt yet. Never auto-adopt it.
    SkipPendingWrite,
    /// Finalize durable receipt before deciding the next release against current state.
    RecoverCommitted,
}
impl TemplateMergeDecision {
    pub fn action(&self) -> &'static str {
        match self {
            Self::Initialize => "create",
            Self::Update => "update",
            Self::Unchanged | Self::RecoverCommitted => "unchanged",
            _ => "skip",
        }
    }
    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Self::SkipUnknownBaseline => Some("unknown_baseline"),
            Self::SkipUserModified => Some("user_modified"),
            Self::SkipUserDeleted => Some("user_deleted"),
            Self::SkipPendingWrite => Some("pending_write"),
            _ => None,
        }
    }
}

use anyhow::Result;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct ProjectedTemplateResource {
    pub key: TemplateResourceKey,
    pub target_id: String,
    /// Only editable portable content, expressed in target identities.
    pub value: Value,
    pub fingerprint: String,
}

/// JSON objects are sorted explicitly even when serde_json is built with preserve_order.
/// Arrays retain their semantic order. Do not recursively strip fields such as created_by:
/// inside editor documents these can be user-defined schema fields.
pub fn template_resource_fingerprint(value: &Value) -> Result<String> {
    fn canonical(value: &Value) -> Value {
        match value {
            Value::Object(object) => {
                let sorted: BTreeMap<_, _> = object
                    .iter()
                    .map(|(k, v)| (k.clone(), canonical(v)))
                    .collect();
                let mut out = serde_json::Map::new();
                for (key, value) in sorted {
                    out.insert(key, value);
                }
                Value::Object(out)
            }
            Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
            _ => value.clone(),
        }
    }
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&canonical(value))?)
    ))
}
