use domain::DataModelScopeKind;
use uuid::Uuid;

pub struct CreateModelDefinitionCommand {
    pub actor_user_id: Uuid,
    pub scope_kind: DataModelScopeKind,
    pub data_source_instance_id: Option<Uuid>,
    pub external_resource_key: Option<String>,
    pub external_table_id: Option<String>,
    pub code: String,
    pub title: String,
    pub status: Option<domain::DataModelStatus>,
}

pub struct PublishModelCommand {
    pub actor_user_id: Uuid,
    pub model_id: Uuid,
}

pub struct UpdateModelDefinitionCommand {
    pub actor_user_id: Uuid,
    pub model_id: Uuid,
    pub title: String,
    pub external_table_id: Option<String>,
}

pub struct UpdateModelDefinitionStatusCommand {
    pub actor_user_id: Uuid,
    pub model_id: Uuid,
    pub status: domain::DataModelStatus,
}

pub struct AddModelFieldCommand {
    pub actor_user_id: Uuid,
    pub model_id: Uuid,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub external_field_key: Option<String>,
    pub field_kind: domain::ModelFieldKind,
    pub is_required: bool,
    pub api_required: Option<bool>,
    pub is_unique: bool,
    pub default_value: Option<serde_json::Value>,
    pub display_interface: Option<String>,
    pub display_options: serde_json::Value,
    pub relation_target_model_id: Option<Uuid>,
    pub relation_options: serde_json::Value,
}

pub struct UpdateModelFieldCommand {
    pub actor_user_id: Uuid,
    pub model_id: Uuid,
    pub field_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub is_required: bool,
    pub api_required: Option<bool>,
    pub is_unique: bool,
    pub default_value: Option<serde_json::Value>,
    pub display_interface: Option<String>,
    pub display_options: serde_json::Value,
    pub relation_options: serde_json::Value,
}

pub struct DeleteModelDefinitionCommand {
    pub actor_user_id: Uuid,
    pub model_id: Uuid,
    pub confirmed: bool,
}

pub struct BatchDeleteModelDefinitionsCommand {
    pub actor_user_id: Uuid,
    pub model_ids: Vec<Uuid>,
    pub confirmed: bool,
}

pub struct DeleteModelFieldCommand {
    pub actor_user_id: Uuid,
    pub model_id: Uuid,
    pub field_id: Uuid,
    pub confirmed: bool,
}

pub struct CreateScopeDataModelGrantCommand {
    pub actor_user_id: Uuid,
    pub scope_kind: DataModelScopeKind,
    pub scope_id: Uuid,
    pub data_model_id: Uuid,
    pub enabled: bool,
    pub permission_profile: String,
    pub confirm_unsafe_external_source_system_all: bool,
}

pub struct UpdateScopeDataModelGrantCommand {
    pub actor_user_id: Uuid,
    pub data_model_id: Uuid,
    pub grant_id: Uuid,
    pub enabled: Option<bool>,
    pub permission_profile: Option<String>,
    pub confirm_unsafe_external_source_system_all: bool,
}

pub struct DeleteScopeDataModelGrantCommand {
    pub actor_user_id: Uuid,
    pub data_model_id: Uuid,
    pub grant_id: Uuid,
}

pub struct PublishedModel {
    pub model: domain::ModelDefinitionRecord,
    pub resource: runtime_core::resource_descriptor::ResourceDescriptor,
}
