use super::*;

pub(super) fn map_instance(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ModelProviderInstanceRecord> {
    PgModelProviderMapper::to_instance_record(StoredModelProviderInstanceRow {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        installation_id: row.get("installation_id"),
        provider_code: row.get("provider_code"),
        protocol: row.get("protocol"),
        display_name: row.get("display_name"),
        status: row.get("status"),
        config_json: row.get("config_json"),
        configured_models_json: row.get("configured_models_json"),
        enabled_model_ids: row.get("enabled_model_ids"),
        included_in_main: row.get("included_in_main"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_main_instance(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ModelProviderMainInstanceRecord> {
    PgModelProviderMapper::to_main_instance_record(StoredModelProviderMainInstanceRow {
        workspace_id: row.get("workspace_id"),
        provider_code: row.get("provider_code"),
        auto_include_new_instances: row.get("auto_include_new_instances"),
        revision: row.get("revision"),
        model_routing_policies: Vec::new(),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_main_model_routing_policy(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ModelProviderMainModelRoutingPolicyRecord> {
    PgModelProviderMapper::to_main_model_routing_policy_record(
        StoredModelProviderMainModelRoutingPolicyRow {
            workspace_id: row.get("workspace_id"),
            provider_code: row.get("provider_code"),
            model_id: row.get("model_id"),
            distribution_rule: row.get("distribution_rule"),
            provider_instance_ids: row.get("provider_instance_ids"),
            created_by: row.get("created_by"),
            updated_by: row.get("updated_by"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        },
    )
}

pub(super) fn map_catalog_cache(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ModelProviderCatalogCacheRecord> {
    PgModelProviderMapper::to_catalog_cache_record(StoredModelProviderCatalogCacheRow {
        provider_instance_id: row.get("provider_instance_id"),
        model_discovery_mode: row.get("model_discovery_mode"),
        refresh_status: row.get("refresh_status"),
        source: row.get("source"),
        models_json: row.get("models_json"),
        last_error_message: row.get("last_error_message"),
        refreshed_at: row.get("refreshed_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_secret(row: sqlx::postgres::PgRow) -> Result<domain::ModelProviderSecretRecord> {
    PgModelProviderMapper::to_secret_record(StoredModelProviderSecretRow {
        provider_instance_id: row.get("provider_instance_id"),
        encrypted_secret_json: row.get("encrypted_secret_json"),
        secret_version: row.get("secret_version"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_preview_session(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ModelProviderPreviewSessionRecord> {
    PgModelProviderMapper::to_preview_session_record(StoredModelProviderPreviewSessionRow {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        actor_user_id: row.get("actor_user_id"),
        installation_id: row.get("installation_id"),
        instance_id: row.get("instance_id"),
        config_fingerprint: row.get("config_fingerprint"),
        models_json: row.get("models_json"),
        expires_at: row.get("expires_at"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn map_catalog_source(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ModelProviderCatalogSourceRecord> {
    Ok(domain::ModelProviderCatalogSourceRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        source_kind: row.get("source_kind"),
        plugin_id: row.get("plugin_id"),
        provider_code: row.get("provider_code"),
        display_name: row.get("display_name"),
        base_url_ref: row.get("base_url_ref"),
        auth_secret_ref: row.get("auth_secret_ref"),
        protocol: row.get("protocol"),
        status: row.get("status"),
        last_sync_run_id: row.get("last_sync_run_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_catalog_sync_run(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ModelCatalogSyncRunRecord> {
    Ok(domain::ModelCatalogSyncRunRecord {
        id: row.get("id"),
        catalog_source_id: row.get("catalog_source_id"),
        status: row.get("status"),
        error_message_ref: row.get("error_message_ref"),
        discovered_count: row.get("discovered_count"),
        imported_count: row.get("imported_count"),
        disabled_count: row.get("disabled_count"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
    })
}

pub(super) fn map_catalog_entry(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ModelProviderCatalogEntryRecord> {
    Ok(domain::ModelProviderCatalogEntryRecord {
        id: row.get("id"),
        provider_instance_id: row.get("provider_instance_id"),
        catalog_source_id: row.get("catalog_source_id"),
        upstream_model_id: row.get("upstream_model_id"),
        display_label: row.get("display_label"),
        protocol: row.get("protocol"),
        capability_snapshot: row.get("capability_snapshot"),
        parameter_schema_ref: row.get("parameter_schema_ref"),
        context_window: row.get("context_window"),
        max_output_tokens: row.get("max_output_tokens"),
        pricing_ref: row.get("pricing_ref"),
        fetched_at: row.get("fetched_at"),
        status: row.get("status"),
    })
}

pub(super) fn map_failover_queue_template(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ModelFailoverQueueTemplateRecord> {
    Ok(domain::ModelFailoverQueueTemplateRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        name: row.get("name"),
        version: row.get("version"),
        status: row.get("status"),
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_failover_queue_item(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ModelFailoverQueueItemRecord> {
    Ok(domain::ModelFailoverQueueItemRecord {
        id: row.get("id"),
        queue_template_id: row.get("queue_template_id"),
        sort_index: row.get("sort_index"),
        provider_instance_id: row.get("provider_instance_id"),
        provider_instance_display_name: row.get("provider_instance_display_name"),
        provider_code: row.get("provider_code"),
        upstream_model_id: row.get("upstream_model_id"),
        protocol: row.get("protocol"),
        enabled: row.get("enabled"),
    })
}

pub(super) fn map_failover_queue_snapshot(
    row: sqlx::postgres::PgRow,
) -> Result<domain::ModelFailoverQueueSnapshotRecord> {
    Ok(domain::ModelFailoverQueueSnapshotRecord {
        id: row.get("id"),
        queue_template_id: row.get("queue_template_id"),
        version: row.get("version"),
        items: row.get("items"),
        created_at: row.get("created_at"),
    })
}
