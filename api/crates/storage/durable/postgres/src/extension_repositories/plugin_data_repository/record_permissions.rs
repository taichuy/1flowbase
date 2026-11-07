//! SQL predicates for the exact Host-admitted resource, before pagination or mutation.
use super::*;
use extension_contracts::{PluginDataRecordPermissionContext, PluginDataRecordResource};

pub(super) struct RequestAuthorization {
    context: PluginDataRecordPermissionContext,
    publisher_namespace: String,
    plugin_code: String,
    caller_workspace_id: Uuid,
    role_ids: Vec<Uuid>,
    pub receipt_fingerprint: serde_json::Value,
}

impl RequestAuthorization {
    pub async fn load(
        transaction: &mut Transaction<'_, Postgres>,
        binding: &PluginDataBinding,
        receipt_required: bool,
    ) -> Result<Option<Self>, PluginDataError> {
        let Some(context) = &binding.record_permissions else {
            return Ok(None);
        };
        let caller_workspace_id = Uuid::parse_str(&context.caller_workspace_id)
            .map_err(|_| PluginDataError::invalid("plugin_data_record_workspace"))?;
        let actor_id = Uuid::parse_str(&context.actor_user_id)
            .map_err(|_| PluginDataError::invalid("plugin_data_record_actor"))?;
        if context.active_role_code.is_empty() {
            return Err(PluginDataError::invalid("plugin_data_record_role"));
        }
        let mut seen = std::collections::BTreeSet::new();
        for resource in &context.resources {
            if !seen.insert(&resource.collection_code)
                || resource.resource_code.is_empty()
                || resource.id_field.is_empty()
                || resource
                    .record_id
                    .as_ref()
                    .is_some_and(|id| id.is_empty() || id.len() > 1024)
            {
                return Err(PluginDataError::invalid("plugin_data_record_resource"));
            }
        }
        let role_ids = if context.is_root {
            vec![]
        } else {
            let effective =
                crate::auth_repository::effective_roles::effective_role_ids_in_transaction(
                    transaction,
                    actor_id,
                    caller_workspace_id,
                    &context.active_role_code,
                )
                .await
                .map_err(storage_error)?;
            sqlx::query_scalar("select id from roles where id=any($1) and scope_kind='workspace' and workspace_id=$2 order by id")
                .bind(effective).bind(caller_workspace_id).fetch_all(&mut **transaction).await.map_err(storage_error)?
        };
        // A receipt never replays a result admitted with another caller or an obsolete policy/role set.
        // Family-wide revisions also invalidate list receipts when a newly restricted row is added.
        let revision: Option<i64> = if receipt_required {
            sqlx::query_scalar("select revision from plugin_record_permission_scope_revisions where workspace_id=$1 and publisher_namespace=$2 and plugin_code=$3")
                .bind(caller_workspace_id).bind(&binding.publisher_namespace).bind(&binding.plugin_code)
                .fetch_optional(&mut **transaction).await.map_err(storage_error)?
        } else {
            None
        };
        let resources: Vec<_> = context
            .resources
            .iter()
            .map(|resource| {
                serde_json::json!([
                    resource.resource_code,
                    resource.collection_code,
                    resource.id_field,
                    resource.action_code,
                    resource.record_id
                ])
            })
            .collect();
        let receipt_fingerprint = serde_json::json!({"workspace_id":caller_workspace_id, "actor_id":actor_id,
            "active_role_code":context.active_role_code, "is_root":context.is_root,
            "role_ids":role_ids, "resources":resources, "policy_revision":revision});
        Ok(Some(Self {
            context: context.clone(),
            publisher_namespace: binding.publisher_namespace.clone(),
            plugin_code: binding.plugin_code.clone(),
            caller_workspace_id,
            role_ids,
            receipt_fingerprint,
        }))
    }

    pub fn for_target(
        &self,
        requested: &PluginDataTarget,
        resolved: &ResolvedTarget,
        operation: &PluginDataOperation,
    ) -> Result<Option<ResolvedRecordPermission>, PluginDataError> {
        let PluginDataTarget::OwnedCollection { collection_code } = requested else {
            return Ok(None);
        };
        let Some(resource) = self
            .context
            .resources
            .iter()
            .find(|resource| &resource.collection_code == collection_code)
        else {
            return Ok(None);
        };
        if resource.action_code.is_empty()
            || (resource.action_code == "create"
                && !matches!(operation, PluginDataOperation::Insert { .. }))
        {
            return Err(error(
                PluginDataErrorKind::PermissionDenied,
                "plugin_data_record_operation",
                false,
            ));
        }
        let id_field = resolved
            .fields
            .get(&resource.id_field)
            .ok_or_else(|| ownership_error("plugin_data_record_identity"))?;
        if !matches!(id_field.field_type.as_str(), "string" | "text" | "uuid") {
            return Err(ownership_error("plugin_data_record_identity_type"));
        }
        Ok(Some(ResolvedRecordPermission {
            resource: resource.clone(),
            publisher_namespace: self.publisher_namespace.clone(),
            plugin_code: self.plugin_code.clone(),
            caller_workspace_id: self.caller_workspace_id,
            role_ids: self.role_ids.clone(),
            is_root: self.context.is_root,
            identity_expression: format!(
                "({}.{})::text",
                quoted(&resolved.table)?,
                quoted(&id_field.physical)?
            ),
        }))
    }
}

pub(super) struct ResolvedRecordPermission {
    resource: PluginDataRecordResource,
    publisher_namespace: String,
    plugin_code: String,
    caller_workspace_id: Uuid,
    role_ids: Vec<Uuid>,
    is_root: bool,
    identity_expression: String,
}

impl ResolvedRecordPermission {
    pub fn push_where(&self, query: &mut QueryBuilder<Postgres>) {
        if let Some(record_id) = &self.resource.record_id {
            query
                .push(" and ")
                .push(&self.identity_expression)
                .push(" = ")
                .push_bind(record_id.clone());
        }
        self.push_policy(query, None);
    }

    fn push_policy(&self, query: &mut QueryBuilder<Postgres>, literal_id: Option<&str>) {
        if self.is_root {
            return;
        }
        query.push(" and not exists (select 1 from plugin_record_permission_policies record_policy where record_policy.workspace_id = ")
            .push_bind(self.caller_workspace_id).push(" and record_policy.publisher_namespace = ").push_bind(self.publisher_namespace.clone())
            .push(" and record_policy.plugin_code = ").push_bind(self.plugin_code.clone())
            .push(" and record_policy.resource_code = ").push_bind(self.resource.resource_code.clone()).push(" and record_policy.record_id = ");
        if let Some(id) = literal_id {
            query.push_bind(id.to_string());
        } else {
            query.push(&self.identity_expression);
        }
        if self.resource.action_code == "create" {
            // Create is never a grantable row action, even for inconsistent durable input.
            query.push(" and record_policy.mode = 'restricted')");
            return;
        }
        query.push(" and record_policy.mode = 'restricted' and not exists (select 1 from plugin_record_permission_grants record_grant join roles record_role on record_role.id=record_grant.role_id where record_grant.policy_id=record_policy.id and record_grant.role_id=any(")
            .push_bind(self.role_ids.clone()).push(") and ").push_bind(self.resource.action_code.clone())
            .push("=any(record_grant.actions) and record_role.scope_kind='workspace' and record_role.workspace_id=")
            .push_bind(self.caller_workspace_id).push("))");
    }

    pub fn validate_update_identity(
        &self,
        values: &BTreeMap<String, PluginDataValue>,
    ) -> Result<(), PluginDataError> {
        if let Some(value) = values.get(&self.resource.id_field) {
            let value = record_value(value)?;
            if self.resource.record_id.as_deref() != Some(value) {
                return Err(error(
                    PluginDataErrorKind::PermissionDenied,
                    "plugin_data_record_identity_mutation",
                    false,
                ));
            }
        }
        Ok(())
    }

    pub fn stable_upsert_values(
        &self,
        identity: &BTreeMap<String, PluginDataValue>,
        values: &BTreeMap<String, PluginDataValue>,
    ) -> Result<BTreeMap<String, PluginDataValue>, PluginDataError> {
        let mut stable = values.clone();
        if let Some(value) = values.get(&self.resource.id_field) {
            if identity.get(&self.resource.id_field) == Some(value) {
                stable.remove(&self.resource.id_field);
            } else {
                self.validate_update_identity(values)?;
            }
        }
        Ok(stable)
    }

    pub async fn authorize_insert(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        values: &BTreeMap<String, PluginDataValue>,
        id: Uuid,
        target: &ResolvedTarget,
        physical_scope_id: Uuid,
    ) -> Result<(), PluginDataError> {
        let generated_id = id.to_string();
        let record_id =
            if self.resource.id_field == "id" {
                generated_id.as_str()
            } else {
                record_value(values.get(&self.resource.id_field).ok_or_else(|| {
                    PluginDataError::invalid("plugin_data_record_identity_missing")
                })?)?
            };
        if self
            .resource
            .record_id
            .as_ref()
            .is_some_and(|bound| bound != record_id)
        {
            return Err(error(
                PluginDataErrorKind::PermissionDenied,
                "plugin_data_record_identity_mismatch",
                false,
            ));
        }
        // Serialize all plugin inserts by stable identity, including duplicates with different row UUIDs.
        let identity_lock = serde_json::to_string(&(
            target.table.as_str(),
            physical_scope_id,
            &self.resource.id_field,
            record_id,
        ))
        .map_err(|_| PluginDataError::invalid("plugin_data_record_identity"))?;
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(identity_lock)
            .execute(&mut **transaction)
            .await
            .map_err(storage_error)?;
        let mut exists = QueryBuilder::<Postgres>::new("select exists(select 1 from ");
        exists
            .push(quoted(&target.table)?)
            .push(" where scope_id=")
            .push_bind(physical_scope_id)
            .push(" and ")
            .push(&self.identity_expression)
            .push("=")
            .push_bind(record_id.to_string())
            .push(")");
        let exists: bool = exists
            .build_query_scalar()
            .fetch_one(&mut **transaction)
            .await
            .map_err(storage_error)?;
        if exists {
            return Err(error(
                PluginDataErrorKind::Conflict,
                "plugin_data_record_exists",
                false,
            ));
        }
        let mut query = QueryBuilder::<Postgres>::new("select true where true");
        self.push_policy(&mut query, Some(record_id));
        let allowed: Option<bool> = query
            .build_query_scalar()
            .fetch_optional(&mut **transaction)
            .await
            .map_err(storage_error)?;
        if allowed == Some(true) {
            Ok(())
        } else {
            Err(error(
                PluginDataErrorKind::PermissionDenied,
                "plugin_data_record_permission",
                false,
            ))
        }
    }
}

fn record_value(value: &PluginDataValue) -> Result<&str, PluginDataError> {
    match value {
        PluginDataValue::String(value) | PluginDataValue::Uuid(value) if !value.is_empty() => {
            Ok(value)
        }
        _ => Err(PluginDataError::invalid(
            "plugin_data_record_identity_value",
        )),
    }
}
