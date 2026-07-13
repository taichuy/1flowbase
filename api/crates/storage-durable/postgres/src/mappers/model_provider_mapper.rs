use anyhow::{anyhow, Result};
use domain::{
    ModelProviderCatalogCacheRecord, ModelProviderCatalogRefreshStatus, ModelProviderCatalogSource,
    ModelProviderDiscoveryMode, ModelProviderDistributionRule, ModelProviderInstanceRecord,
    ModelProviderInstanceStatus, ModelProviderMainInstanceRecord, ModelProviderSecretRecord,
};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StoredModelProviderInstanceRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub installation_id: Uuid,
    pub provider_code: String,
    pub protocol: String,
    pub display_name: String,
    pub status: String,
    pub config_json: serde_json::Value,
    pub configured_models_json: serde_json::Value,
    pub enabled_model_ids: Vec<String>,
    pub included_in_main: bool,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredModelProviderMainInstanceRow {
    pub workspace_id: Uuid,
    pub provider_code: String,
    pub auto_include_new_instances: bool,
    pub model_distribution_rules: Vec<domain::ModelProviderMainModelDistributionRuleRecord>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredModelProviderMainModelDistributionRuleRow {
    pub workspace_id: Uuid,
    pub provider_code: String,
    pub model_id: String,
    pub distribution_rule: String,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredModelProviderCatalogCacheRow {
    pub provider_instance_id: Uuid,
    pub model_discovery_mode: String,
    pub refresh_status: String,
    pub source: String,
    pub models_json: serde_json::Value,
    pub last_error_message: Option<String>,
    pub refreshed_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredModelProviderSecretRow {
    pub provider_instance_id: Uuid,
    pub encrypted_secret_json: serde_json::Value,
    pub secret_version: i32,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredModelProviderPreviewSessionRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub installation_id: Option<Uuid>,
    pub instance_id: Option<Uuid>,
    pub config_fingerprint: String,
    pub models_json: serde_json::Value,
    pub expires_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
}

pub struct PgModelProviderMapper;

impl PgModelProviderMapper {
    pub fn to_instance_record(
        row: StoredModelProviderInstanceRow,
    ) -> Result<ModelProviderInstanceRecord> {
        Ok(ModelProviderInstanceRecord {
            id: row.id,
            workspace_id: row.workspace_id,
            installation_id: row.installation_id,
            provider_code: row.provider_code,
            protocol: row.protocol,
            display_name: row.display_name,
            status: parse_instance_status(&row.status)?,
            config_json: row.config_json,
            configured_models: serde_json::from_value(row.configured_models_json)?,
            enabled_model_ids: row.enabled_model_ids,
            included_in_main: row.included_in_main,
            created_by: row.created_by,
            updated_by: row.updated_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub fn to_main_instance_record(
        row: StoredModelProviderMainInstanceRow,
    ) -> Result<ModelProviderMainInstanceRecord> {
        Ok(ModelProviderMainInstanceRecord {
            workspace_id: row.workspace_id,
            provider_code: row.provider_code,
            auto_include_new_instances: row.auto_include_new_instances,
            model_distribution_rules: row.model_distribution_rules,
            created_by: row.created_by,
            updated_by: row.updated_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub fn to_main_model_distribution_rule_record(
        row: StoredModelProviderMainModelDistributionRuleRow,
    ) -> Result<domain::ModelProviderMainModelDistributionRuleRecord> {
        Ok(domain::ModelProviderMainModelDistributionRuleRecord {
            workspace_id: row.workspace_id,
            provider_code: row.provider_code,
            model_id: row.model_id,
            distribution_rule: parse_distribution_rule(&row.distribution_rule)?,
            created_by: row.created_by,
            updated_by: row.updated_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub fn to_catalog_cache_record(
        row: StoredModelProviderCatalogCacheRow,
    ) -> Result<ModelProviderCatalogCacheRecord> {
        Ok(ModelProviderCatalogCacheRecord {
            provider_instance_id: row.provider_instance_id,
            model_discovery_mode: parse_discovery_mode(&row.model_discovery_mode)?,
            refresh_status: parse_refresh_status(&row.refresh_status)?,
            source: parse_catalog_source(&row.source)?,
            models_json: row.models_json,
            last_error_message: row.last_error_message,
            refreshed_at: row.refreshed_at,
            updated_at: row.updated_at,
        })
    }

    pub fn to_secret_record(
        row: StoredModelProviderSecretRow,
    ) -> Result<ModelProviderSecretRecord> {
        Ok(ModelProviderSecretRecord {
            provider_instance_id: row.provider_instance_id,
            encrypted_secret_json: row.encrypted_secret_json,
            secret_version: row.secret_version,
            updated_at: row.updated_at,
        })
    }

    pub fn to_preview_session_record(
        row: StoredModelProviderPreviewSessionRow,
    ) -> Result<domain::ModelProviderPreviewSessionRecord> {
        Ok(domain::ModelProviderPreviewSessionRecord {
            id: row.id,
            workspace_id: row.workspace_id,
            actor_user_id: row.actor_user_id,
            installation_id: row.installation_id,
            instance_id: row.instance_id,
            config_fingerprint: row.config_fingerprint,
            models_json: row.models_json,
            expires_at: row.expires_at,
            created_at: row.created_at,
        })
    }
}

pub fn parse_instance_status(value: &str) -> Result<ModelProviderInstanceStatus> {
    match value {
        "draft" => Ok(ModelProviderInstanceStatus::Draft),
        "ready" => Ok(ModelProviderInstanceStatus::Ready),
        "invalid" => Ok(ModelProviderInstanceStatus::Invalid),
        "disabled" => Ok(ModelProviderInstanceStatus::Disabled),
        _ => Err(anyhow!("unknown model provider instance status: {value}")),
    }
}

pub fn parse_distribution_rule(value: &str) -> Result<ModelProviderDistributionRule> {
    match value {
        "none" => Ok(ModelProviderDistributionRule::None),
        "round_robin" => Ok(ModelProviderDistributionRule::RoundRobin),
        "retry_round_robin" => Ok(ModelProviderDistributionRule::RetryRoundRobin),
        _ => Err(anyhow!("unknown model provider distribution rule: {value}")),
    }
}

pub fn parse_discovery_mode(value: &str) -> Result<ModelProviderDiscoveryMode> {
    match value {
        "static" => Ok(ModelProviderDiscoveryMode::Static),
        "dynamic" => Ok(ModelProviderDiscoveryMode::Dynamic),
        "hybrid" => Ok(ModelProviderDiscoveryMode::Hybrid),
        _ => Err(anyhow!("unknown model provider discovery mode: {value}")),
    }
}

pub fn parse_refresh_status(value: &str) -> Result<ModelProviderCatalogRefreshStatus> {
    match value {
        "idle" => Ok(ModelProviderCatalogRefreshStatus::Idle),
        "refreshing" => Ok(ModelProviderCatalogRefreshStatus::Refreshing),
        "ready" => Ok(ModelProviderCatalogRefreshStatus::Ready),
        "failed" => Ok(ModelProviderCatalogRefreshStatus::Failed),
        _ => Err(anyhow!("unknown model provider refresh status: {value}")),
    }
}

pub fn parse_catalog_source(value: &str) -> Result<ModelProviderCatalogSource> {
    match value {
        "static" => Ok(ModelProviderCatalogSource::Static),
        "dynamic" => Ok(ModelProviderCatalogSource::Dynamic),
        "hybrid" => Ok(ModelProviderCatalogSource::Hybrid),
        _ => Err(anyhow!("unknown model provider catalog source: {value}")),
    }
}
