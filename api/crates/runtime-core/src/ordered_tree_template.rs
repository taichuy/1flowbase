use plugin_framework::{
    DataModelCapabilityRequirement, DataModelOperationHandlerRef, DataModelOperationMethod,
    DataModelSystemFieldWritePolicy, DataModelTemplateDescriptor, DataModelTemplateIdentity,
    DataModelTemplateOperation, DataModelTemplateSourceSelector, DataModelTemplateSystemField,
    DATA_MODEL_TEMPLATE_DESCRIPTOR_VERSION_V1,
};
use serde_json::{json, Value};

use crate::general_data_model_template::GENERAL_RECORDS_READ_CAPABILITY;

pub const ORDERED_TREE_TEMPLATE_CODE: &str = "ordered_tree";
pub const ORDERED_TREE_TEMPLATE_VERSION: &str = "v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreOrderedTreeOperationHandler {
    ListRecords,
    GetRecord,
    CreateRecord,
    UpdateRecord,
    DeleteRecord,
    ListRoots,
    ListChildren,
    ListAncestors,
    ListDescendants,
    Search,
    Move,
    DeleteSubtree,
}

impl CoreOrderedTreeOperationHandler {
    pub fn from_ref(handler_ref: &DataModelOperationHandlerRef) -> Option<Self> {
        if handler_ref.provider != "core" || handler_ref.version != "v1" {
            return None;
        }
        match handler_ref.code.as_str() {
            "ordered_tree_list_records" => Some(Self::ListRecords),
            "ordered_tree_get_record" => Some(Self::GetRecord),
            "ordered_tree_create_record" => Some(Self::CreateRecord),
            "ordered_tree_update_record" => Some(Self::UpdateRecord),
            "ordered_tree_delete_record" => Some(Self::DeleteRecord),
            "ordered_tree_list_roots" => Some(Self::ListRoots),
            "ordered_tree_list_children" => Some(Self::ListChildren),
            "ordered_tree_list_ancestors" => Some(Self::ListAncestors),
            "ordered_tree_list_descendants" => Some(Self::ListDescendants),
            "ordered_tree_search" => Some(Self::Search),
            "ordered_tree_move" => Some(Self::Move),
            "ordered_tree_delete_subtree" => Some(Self::DeleteSubtree),
            _ => None,
        }
    }

    pub fn audit_action(self) -> &'static str {
        match self {
            Self::ListRecords => "list",
            Self::GetRecord => "get",
            Self::CreateRecord => "create",
            Self::UpdateRecord => "update",
            Self::DeleteRecord => "delete",
            Self::ListRoots => "tree.roots",
            Self::ListChildren => "tree.children",
            Self::ListAncestors => "tree.ancestors",
            Self::ListDescendants => "tree.descendants",
            Self::Search => "tree.search",
            Self::Move => "tree.move",
            Self::DeleteSubtree => "tree.delete_subtree",
        }
    }
}

pub fn ordered_tree_template_identity() -> DataModelTemplateIdentity {
    DataModelTemplateIdentity {
        provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.to_owned(),
        code: ORDERED_TREE_TEMPLATE_CODE.to_owned(),
        version: ORDERED_TREE_TEMPLATE_VERSION.to_owned(),
    }
}

pub(crate) fn ordered_tree_template_descriptor() -> DataModelTemplateDescriptor {
    DataModelTemplateDescriptor {
        descriptor_version: DATA_MODEL_TEMPLATE_DESCRIPTOR_VERSION_V1,
        identity: ordered_tree_template_identity(),
        source_selector: DataModelTemplateSourceSelector::MainSource,
        required_capabilities: vec![DataModelCapabilityRequirement {
            code: GENERAL_RECORDS_READ_CAPABILITY.to_owned(),
        }],
        system_fields: vec![
            system_field(
                "id",
                json!({ "type": "string", "format": "uuid" }),
                true,
                DataModelSystemFieldWritePolicy::RuntimeGenerated,
            ),
            system_field(
                "scope_id",
                json!({ "type": "string", "format": "uuid" }),
                true,
                DataModelSystemFieldWritePolicy::RuntimeManaged,
            ),
            system_field(
                "created_by",
                nullable_uuid_schema(),
                false,
                DataModelSystemFieldWritePolicy::RuntimeManaged,
            ),
            system_field(
                "updated_by",
                nullable_uuid_schema(),
                false,
                DataModelSystemFieldWritePolicy::RuntimeManaged,
            ),
            system_field(
                "created_at",
                json!({ "type": "string", "format": "date-time" }),
                true,
                DataModelSystemFieldWritePolicy::DatabaseGenerated,
            ),
            system_field(
                "updated_at",
                json!({ "type": "string", "format": "date-time" }),
                true,
                DataModelSystemFieldWritePolicy::DatabaseManaged,
            ),
            system_field(
                "parent_id",
                nullable_uuid_schema(),
                false,
                DataModelSystemFieldWritePolicy::RuntimeManaged,
            ),
            system_field(
                "sibling_rank",
                json!({ "type": "string", "readOnly": true }),
                true,
                DataModelSystemFieldWritePolicy::RuntimeManaged,
            ),
        ],
        operations: vec![
            operation("list_records", DataModelOperationMethod::Get, "/api/runtime/models/{model_code}/list", "view", list_records_schema(), list_records_output_schema(), "ordered_tree_list_records", "List tree records", "List ordered-tree records through the existing CRUD path."),
            operation("get_record", DataModelOperationMethod::Get, "/api/runtime/models/{model_code}/get/{id}", "view", get_record_schema(), object_schema(), "ordered_tree_get_record", "Get tree record", "Get one ordered-tree record through the existing CRUD path."),
            operation("create_record", DataModelOperationMethod::Post, "/api/runtime/models/{model_code}/create", "create", create_schema(), object_schema(), "ordered_tree_create_record", "Create tree node", "Create a tree node at a parent-relative position; omitted position appends a root."),
            operation("update_record", DataModelOperationMethod::Patch, "/api/runtime/models/{model_code}/update/{id}", "update", business_update_schema(), object_schema(), "ordered_tree_update_record", "Update tree node", "Update business fields without changing tree structure."),
            operation("delete_record", DataModelOperationMethod::Delete, "/api/runtime/models/{model_code}/delete/{id}", "delete", json!({ "type": "object" }), json!({ "type": "object", "required": ["deleted"], "properties": { "deleted": { "type": "boolean" } } }), "ordered_tree_delete_record", "Delete tree leaf", "Delete a node only when it has no children."),
            operation("tree_roots", DataModelOperationMethod::Get, "/api/runtime/models/{model_code}/tree/roots", "view", limit_schema(), array_schema(), "ordered_tree_list_roots", "List tree roots", "List roots in stable sibling order."),
            operation("tree_children", DataModelOperationMethod::Get, "/api/runtime/models/{model_code}/tree/children/{id}", "view", limit_schema(), array_schema(), "ordered_tree_list_children", "List tree children", "List direct children in stable sibling order."),
            operation("tree_ancestors", DataModelOperationMethod::Get, "/api/runtime/models/{model_code}/tree/ancestors/{id}", "view", json!({ "type": "object" }), array_schema(), "ordered_tree_list_ancestors", "List tree ancestors", "List ancestors from root to direct parent."),
            operation("tree_descendants", DataModelOperationMethod::Get, "/api/runtime/models/{model_code}/tree/descendants/{id}", "view", descendants_schema(), array_schema(), "ordered_tree_list_descendants", "List tree descendants", "List bounded descendants with depth and child markers."),
            operation("tree_search", DataModelOperationMethod::Get, "/api/runtime/models/{model_code}/tree/search", "view", search_schema(), array_schema(), "ordered_tree_search", "Search tree", "Search a case-insensitive business-text prefix and return ancestor context."),
            operation("tree_move", DataModelOperationMethod::Post, "/api/runtime/models/{model_code}/tree/move/{id}", "update", move_schema(), object_schema(), "ordered_tree_move", "Move tree node", "Move a node to a parent-relative position."),
            operation("tree_delete_subtree", DataModelOperationMethod::Post, "/api/runtime/models/{model_code}/tree/delete-subtree/{id}", "delete", json!({ "type": "object", "required": ["expected_affected_count"], "properties": { "expected_affected_count": { "type": "integer", "minimum": 1 } }, "additionalProperties": false }), json!({ "type": "object", "required": ["deleted_count"], "properties": { "deleted_count": { "type": "integer" } } }), "ordered_tree_delete_subtree", "Delete tree subtree", "Delete a subtree only when its current count matches the caller expectation."),
        ],
        summary: "Ordered tree data model".to_owned(),
        description: "Core main-source ordered-tree Data Model template.".to_owned(),
    }
}

#[allow(clippy::too_many_arguments)]
fn operation(
    code: &str,
    method: DataModelOperationMethod,
    path: &str,
    permission_action: &str,
    input_schema: Value,
    output_schema: Value,
    handler_code: &str,
    summary: &str,
    description: &str,
) -> DataModelTemplateOperation {
    DataModelTemplateOperation {
        code: code.to_owned(),
        method,
        path: path.to_owned(),
        input_schema,
        output_schema,
        permission_action: permission_action.to_owned(),
        handler_ref: DataModelOperationHandlerRef {
            provider: "core".to_owned(),
            code: handler_code.to_owned(),
            version: "v1".to_owned(),
        },
        summary: summary.to_owned(),
        description: description.to_owned(),
    }
}

fn system_field(
    code: &str,
    value_schema: Value,
    required: bool,
    write_policy: DataModelSystemFieldWritePolicy,
) -> DataModelTemplateSystemField {
    DataModelTemplateSystemField {
        code: code.to_owned(),
        value_schema,
        required,
        write_policy,
        summary: format!("{code} system field"),
        description: format!("Core-managed `{code}` field."),
    }
}

fn nullable_uuid_schema() -> Value {
    json!({ "anyOf": [{ "type": "string", "format": "uuid" }, { "type": "null" }] })
}
fn object_schema() -> Value {
    json!({ "type": "object" })
}
fn array_schema() -> Value {
    json!({ "type": "array", "items": { "type": "object" } })
}
fn list_records_schema() -> Value {
    json!({ "type": "object", "properties": { "filter": { "type": "string" }, "sort": { "type": "string" }, "expand": { "type": "string" }, "page": { "type": "integer", "minimum": 1, "default": 1 }, "page_size": { "type": "integer", "minimum": 1, "default": 20 } } })
}
fn get_record_schema() -> Value {
    json!({ "type": "object", "properties": { "expand": { "type": "string" } } })
}
fn list_records_output_schema() -> Value {
    json!({ "type": "object", "required": ["items", "total"], "properties": { "items": { "type": "array", "items": { "type": "object" } }, "total": { "type": "integer" } } })
}
fn nullable_uuid_property() -> Value {
    json!({ "anyOf": [{ "type": "string", "format": "uuid" }, { "type": "null" }] })
}
fn position_properties() -> Value {
    json!({ "parent_id": nullable_uuid_property(), "before_id": { "type": ["string", "null"], "format": "uuid" }, "after_id": { "type": ["string", "null"], "format": "uuid" } })
}
fn create_schema() -> Value {
    json!({ "type": "object", "properties": position_properties(), "additionalProperties": true })
}
fn business_update_schema() -> Value {
    json!({ "type": "object", "not": { "anyOf": [{ "required": ["parent_id"] }, { "required": ["sibling_rank"] }] }, "additionalProperties": true })
}
fn limit_schema() -> Value {
    json!({ "type": "object", "properties": { "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 100 } } })
}
fn descendants_schema() -> Value {
    json!({ "type": "object", "properties": { "max_depth": { "type": "integer", "minimum": 1, "maximum": 256, "default": 32 }, "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 100 }, "include_path": { "type": "boolean", "default": false } } })
}
fn search_schema() -> Value {
    json!({ "type": "object", "required": ["prefix"], "properties": { "prefix": { "type": "string", "minLength": 1 }, "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 } } })
}
fn move_schema() -> Value {
    json!({ "type": "object", "properties": { "new_parent_id": nullable_uuid_property(), "before_id": { "type": ["string", "null"], "format": "uuid" }, "after_id": { "type": ["string", "null"], "format": "uuid" } }, "additionalProperties": false })
}
