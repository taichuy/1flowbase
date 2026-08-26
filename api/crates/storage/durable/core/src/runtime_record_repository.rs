use anyhow::Result;
use async_trait::async_trait;
use domain::ResourceFilterExpr;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::model_metadata::ModelMetadata;

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeSortInput {
    pub field_code: String,
    pub direction: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeListResult {
    pub items: Vec<Value>,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeListQuery {
    pub scope_id: Option<Uuid>,
    pub owner_user_id: Option<Uuid>,
    pub filter: ResourceFilterExpr,
    pub sorts: Vec<RuntimeSortInput>,
    pub expand_relations: Vec<String>,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedTreeCreatePosition {
    pub parent_id: Option<Uuid>,
    pub before_id: Option<Uuid>,
    pub after_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedTreeMovePosition {
    pub new_parent_id: Option<Uuid>,
    pub before_id: Option<Uuid>,
    pub after_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderedTreeCreateInput {
    pub actor_user_id: Uuid,
    pub scope_id: Uuid,
    pub tree_partition_id: Uuid,
    pub payload: Value,
    pub position: OrderedTreeCreatePosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedTreeCreateResult {
    pub node_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedTreeMoveInput {
    pub actor_user_id: Uuid,
    pub scope_id: Uuid,
    pub tree_partition_id: Uuid,
    pub node_id: Uuid,
    pub position: OrderedTreeMovePosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedTreeLeafDeleteInput {
    pub scope_id: Uuid,
    pub tree_partition_id: Uuid,
    pub node_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedTreeSubtreeDeleteInput {
    pub scope_id: Uuid,
    pub tree_partition_id: Uuid,
    pub node_id: Uuid,
    pub expected_affected_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedTreeSubtreeDeleteResult {
    pub deleted_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedTreeBoundedListInput {
    pub scope_id: Uuid,
    pub tree_partition_id: Uuid,
    pub result_limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedTreeChildrenInput {
    pub scope_id: Uuid,
    pub tree_partition_id: Uuid,
    pub parent_id: Uuid,
    pub result_limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedTreeNodeInput {
    pub scope_id: Uuid,
    pub tree_partition_id: Uuid,
    pub node_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedTreeSubtreeImpactInput {
    pub scope_id: Uuid,
    pub tree_partition_id: Uuid,
    pub node_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedTreeSubtreeImpactResult {
    pub affected_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedTreeDescendantsInput {
    pub scope_id: Uuid,
    pub tree_partition_id: Uuid,
    pub node_id: Uuid,
    pub max_depth: u32,
    pub result_limit: u32,
    pub include_path: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedTreeSearchInput {
    pub scope_id: Uuid,
    pub tree_partition_id: Uuid,
    pub prefix: String,
    pub match_limit: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderedTreeNodeProjection {
    pub record: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderedTreeDescendantProjection {
    pub record: Value,
    pub depth: u32,
    pub has_children: bool,
    pub path: Option<Vec<Uuid>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderedTreeSearchProjection {
    pub record: Value,
    pub is_match: bool,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OrderedTreeCommandError {
    #[error("ordered-tree command requires core/ordered_tree/v1 metadata")]
    WrongTemplate,
    #[error("ordered-tree position must specify at most one anchor")]
    ConflictingAnchors,
    #[error("ordered-tree node not found")]
    NodeNotFound,
    #[error("ordered-tree parent not found in scope")]
    ParentNotFound,
    #[error("ordered-tree anchor not found in scope")]
    AnchorNotFound,
    #[error("ordered-tree anchor does not belong to the target sibling group")]
    AnchorSiblingGroupConflict,
    #[error("ordered-tree move would create a cycle")]
    Cycle,
    #[error("tree_node_has_children")]
    TreeNodeHasChildren,
    #[error("ordered-tree subtree changed: expected {expected}, found {actual}")]
    ExpectedAffectedCountMismatch { expected: u64, actual: u64 },
    #[error("ordered-tree sibling position conflicts with a concurrent write")]
    PositionConflict,
    #[error("ordered-tree payload field is not writable: {0}")]
    FieldNotWritable(String),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OrderedTreeQueryError {
    #[error("ordered-tree query requires core/ordered_tree/v1 metadata")]
    WrongTemplate,
    #[error("ordered-tree query node not found")]
    NodeNotFound,
    #[error("ordered-tree query parent not found")]
    ParentNotFound,
    #[error("ordered-tree query result limit must be between 1 and {max}")]
    InvalidResultLimit { max: u32 },
    #[error("ordered-tree descendant depth must be between 1 and {max}")]
    InvalidMaxDepth { max: u32 },
    #[error("ordered-tree ancestor depth exceeds hard limit {max}")]
    AncestorDepthLimitExceeded { max: u32 },
    #[error("ordered-tree search prefix must not be empty")]
    EmptySearchPrefix,
    #[error("ordered-tree model has no searchable text fields")]
    NoSearchableFields,
}

impl OrderedTreeCommandError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::TreeNodeHasChildren => "tree_node_has_children",
            Self::ExpectedAffectedCountMismatch { .. } => "tree_subtree_changed",
            Self::PositionConflict => "tree_position_conflict",
            Self::Cycle => "tree_cycle",
            Self::NodeNotFound => "tree_node_not_found",
            Self::ParentNotFound => "tree_parent_not_found",
            Self::AnchorNotFound => "tree_anchor_not_found",
            Self::AnchorSiblingGroupConflict => "tree_anchor_sibling_group_conflict",
            Self::ConflictingAnchors => "tree_conflicting_anchors",
            Self::WrongTemplate => "tree_wrong_template",
            Self::FieldNotWritable(_) => "tree_field_not_writable",
        }
    }
}

#[async_trait]
pub trait OrderedTreeStructureRepository: Send + Sync {
    async fn create_ordered_tree_node(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeCreateInput,
    ) -> Result<OrderedTreeCreateResult>;

    async fn move_ordered_tree_node(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeMoveInput,
    ) -> Result<()>;

    async fn delete_ordered_tree_leaf(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeLeafDeleteInput,
    ) -> Result<bool>;

    async fn delete_ordered_tree_subtree(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeSubtreeDeleteInput,
    ) -> Result<OrderedTreeSubtreeDeleteResult>;
}

#[async_trait]
pub trait OrderedTreeQueryRepository: Send + Sync {
    async fn get_ordered_tree_subtree_impact(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeSubtreeImpactInput,
    ) -> Result<OrderedTreeSubtreeImpactResult>;

    async fn list_ordered_tree_roots(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeBoundedListInput,
    ) -> Result<Vec<OrderedTreeNodeProjection>>;

    async fn list_ordered_tree_children(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeChildrenInput,
    ) -> Result<Vec<OrderedTreeNodeProjection>>;

    async fn list_ordered_tree_ancestors(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeNodeInput,
    ) -> Result<Vec<OrderedTreeNodeProjection>>;

    async fn list_ordered_tree_descendants(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeDescendantsInput,
    ) -> Result<Vec<OrderedTreeDescendantProjection>>;

    async fn search_ordered_tree_prefix(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeSearchInput,
    ) -> Result<Vec<OrderedTreeSearchProjection>>;
}

pub trait OrderedTreeRuntimeRepository:
    OrderedTreeStructureRepository + OrderedTreeQueryRepository
{
}

impl<T> OrderedTreeRuntimeRepository for T where
    T: OrderedTreeStructureRepository + OrderedTreeQueryRepository
{
}

#[async_trait]
pub trait RuntimeRecordRepository: Send + Sync {
    async fn list_records(
        &self,
        metadata: &ModelMetadata,
        query: RuntimeListQuery,
    ) -> Result<RuntimeListResult>;
    async fn get_record(
        &self,
        metadata: &ModelMetadata,
        scope_id: Option<uuid::Uuid>,
        owner_user_id: Option<uuid::Uuid>,
        record_id: &str,
    ) -> Result<Option<Value>>;
    async fn create_record(
        &self,
        metadata: &ModelMetadata,
        actor_user_id: uuid::Uuid,
        scope_id: uuid::Uuid,
        payload: Value,
    ) -> Result<Value>;
    async fn update_record(
        &self,
        metadata: &ModelMetadata,
        actor_user_id: uuid::Uuid,
        scope_id: Option<uuid::Uuid>,
        owner_user_id: Option<uuid::Uuid>,
        record_id: &str,
        payload: Value,
    ) -> Result<Value>;
    async fn delete_record(
        &self,
        metadata: &ModelMetadata,
        scope_id: Option<uuid::Uuid>,
        owner_user_id: Option<uuid::Uuid>,
        record_id: &str,
    ) -> Result<bool>;
}
