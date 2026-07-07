use super::*;

#[async_trait]
impl ApplicationApiMappingRepository for ApplicationPublicApiTestRepository {
    async fn get_application_api_mapping(
        &self,
        application_id: Uuid,
    ) -> Result<Option<mapping::ApplicationApiMappingConfig>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .mappings
            .get(&application_id)
            .cloned())
    }

    async fn load_application_api_mapping_application_id_by_extension_slug(
        &self,
        slug: &str,
    ) -> Result<Option<Uuid>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .mappings
            .iter()
            .find(|(_, mapping)| mapping.extension_slug() == Some(slug))
            .map(|(application_id, _)| *application_id))
    }

    async fn replace_application_api_mapping(
        &self,
        input: &ReplaceApplicationApiMappingInput,
    ) -> Result<mapping::ApplicationApiMappingConfig> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        if !inner.applications.contains_key(&input.application_id) {
            return Err(ControlPlaneError::NotFound("application").into());
        }
        if let Some(slug) = input.mapping.extension_slug() {
            if inner.mappings.iter().any(|(application_id, mapping)| {
                *application_id != input.application_id && mapping.extension_slug() == Some(slug)
            }) {
                return Err(ControlPlaneError::Conflict("extension_slug").into());
            }
        }
        inner
            .mappings
            .insert(input.application_id, input.mapping.clone());
        Ok(input.mapping.clone())
    }
}

#[async_trait]
impl ApplicationCompileContextRepository for ApplicationPublicApiTestRepository {
    async fn build_application_compile_context(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<orchestration_runtime::compiler::FlowCompileContext> {
        let js_dependencies = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .js_dependency_selections
            .values()
            .filter(|selection| {
                selection.workspace_id == workspace_id && selection.application_id == application_id
            })
            .map(|selection| {
                (
                    orchestration_runtime::compiler::js_dependency_lookup_key(
                        &selection.target,
                        &selection.alias,
                    ),
                    orchestration_runtime::compiler::FlowCompileJsDependency {
                        alias: selection.alias.clone(),
                        target: selection.target.clone(),
                        artifact_path: selection.artifact_path.clone(),
                        artifact_hash: selection.artifact_hash.clone(),
                        integrity: selection.integrity.clone(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        Ok(orchestration_runtime::compiler::FlowCompileContext {
            workspace_id: Some(workspace_id),
            provider_families: Default::default(),
            provider_instances: Default::default(),
            model_distribution_rules: Default::default(),
            node_contributions: Default::default(),
            js_dependencies,
        })
    }
}

#[async_trait]
impl ApplicationCompiledPlanRepository for ApplicationPublicApiTestRepository {
    async fn upsert_application_compiled_plan(
        &self,
        input: &UpsertCompiledPlanInput,
    ) -> Result<domain::CompiledPlanRecord> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        inner.next_compiled_plan_ordinal += 1;
        let now = OffsetDateTime::UNIX_EPOCH
            + time::Duration::seconds(inner.next_compiled_plan_ordinal as i64);
        let record = domain::CompiledPlanRecord {
            id: deterministic_test_id(
                0x55555555555555550000000000000000,
                inner.next_compiled_plan_ordinal,
            ),
            flow_id: input.flow_id,
            draft_id: input.flow_draft_id,
            schema_version: input.schema_version.clone(),
            document_hash: input.document_hash.clone(),
            document_updated_at: input.document_updated_at,
            plan: input.plan.clone(),
            created_by: input.actor_user_id,
            created_at: now,
            updated_at: now,
        };
        inner.compiled_plans.insert(record.id, record.clone());
        Ok(record)
    }

    async fn get_application_compiled_plan(
        &self,
        compiled_plan_id: Uuid,
    ) -> Result<Option<domain::CompiledPlanRecord>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .compiled_plans
            .get(&compiled_plan_id)
            .cloned())
    }
}
