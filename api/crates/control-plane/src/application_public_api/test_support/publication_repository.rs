use super::*;

#[async_trait]
impl ApplicationPublicationRepository for ApplicationPublicApiTestRepository {
    async fn create_active_application_publication_version(
        &self,
        input: &CreateApplicationPublicationVersionInput,
    ) -> Result<publications::ApplicationPublicationVersionRecord> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");

        if !inner.applications.contains_key(&input.application_id) {
            return Err(ControlPlaneError::NotFound("application").into());
        }
        if let Some(slug) = input.extension_slug.as_deref() {
            if inner.publications.values().any(|publication| {
                publication.application_id != input.application_id
                    && publication.extension_slug.as_deref() == Some(slug)
            }) {
                return Err(ControlPlaneError::Conflict("extension_slug").into());
            }
        }

        let existing_id = inner
            .publications
            .values()
            .find(|publication| publication.application_id == input.application_id)
            .map(|publication| publication.id);
        inner.next_publication_ordinal += 1;
        let ordinal = inner.next_publication_ordinal;

        let publication = publications::ApplicationPublicationVersionRecord {
            id: existing_id.unwrap_or_else(|| {
                deterministic_test_id(0x44444444444444440000000000000000, ordinal)
            }),
            application_id: input.application_id,
            flow_id: input.flow_id,
            flow_version_id: input.flow_version_id,
            mapping_snapshot: input.mapping_snapshot.clone(),
            extension_slug: input.extension_slug.clone(),
            compiled_plan_id: input.compiled_plan_id,
            version_sequence: 1,
            active: true,
            api_enabled: input.api_enabled,
            flow_schema_version: input.flow_schema_version.clone(),
            document_hash: input.document_hash.clone(),
            document_snapshot: input.document_snapshot.clone(),
            runtime_profile_snapshot: input.runtime_profile_snapshot.clone(),
            output_selector: input.output_selector.clone(),
            dependency_snapshot: input.dependency_snapshot.clone(),
            created_by: input.actor_user_id,
            created_at: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(ordinal as i64),
        };
        inner
            .application_api_enabled
            .insert(input.application_id, input.api_enabled);
        inner
            .publications
            .insert(publication.id, publication.clone());

        Ok(publication)
    }

    async fn get_application_publication_version(
        &self,
        publication_id: Uuid,
    ) -> Result<Option<publications::ApplicationPublicationVersionRecord>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .publications
            .get(&publication_id)
            .cloned())
    }

    async fn list_application_publication_versions(
        &self,
        application_id: Uuid,
    ) -> Result<Vec<publications::ApplicationPublicationVersionRecord>> {
        let mut publications = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .publications
            .values()
            .filter(|publication| publication.application_id == application_id)
            .cloned()
            .collect::<Vec<_>>();
        publications.sort_by(|left, right| {
            right
                .version_sequence
                .cmp(&left.version_sequence)
                .then(right.id.cmp(&left.id))
        });
        Ok(publications)
    }

    async fn load_active_application_publication(
        &self,
        application_id: Uuid,
    ) -> Result<Option<publications::ApplicationPublicationVersionRecord>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .publications
            .values()
            .find(|publication| publication.application_id == application_id && publication.active)
            .cloned())
    }

    async fn load_active_application_publication_by_extension_slug(
        &self,
        slug: &str,
    ) -> Result<Option<publications::ApplicationPublicationVersionRecord>> {
        Ok(self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .publications
            .values()
            .find(|publication| {
                publication.active && publication.extension_slug.as_deref() == Some(slug)
            })
            .cloned())
    }

    async fn list_enabled_extension_publications(
        &self,
    ) -> Result<Vec<publications::ApplicationPublicationVersionRecord>> {
        let inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        let mut publications = inner
            .publications
            .values()
            .filter(|publication| {
                publication.active
                    && publication.api_enabled
                    && publication.extension_slug.as_deref().is_some()
                    && inner
                        .applications
                        .get(&publication.application_id)
                        .is_some_and(|application| {
                            application.workflow_trigger_type
                                == Some(domain::WorkflowTriggerType::Extension)
                        })
            })
            .cloned()
            .collect::<Vec<_>>();
        publications.sort_by(|left, right| left.extension_slug.cmp(&right.extension_slug));
        Ok(publications)
    }

    async fn set_application_api_enabled(
        &self,
        input: &SetApplicationApiEnabledInput,
    ) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        if !inner.applications.contains_key(&input.application_id) {
            return Err(ControlPlaneError::NotFound("application").into());
        }
        inner
            .application_api_enabled
            .insert(input.application_id, input.api_enabled);
        for publication in inner.publications.values_mut() {
            if publication.application_id == input.application_id {
                publication.api_enabled = input.api_enabled;
            }
        }
        Ok(())
    }

    async fn deactivate_application_publication_versions(
        &self,
        input: &DeactivateApplicationPublicationsInput,
    ) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        if !inner.applications.contains_key(&input.application_id) {
            return Err(ControlPlaneError::NotFound("application").into());
        }
        inner
            .application_api_enabled
            .insert(input.application_id, false);
        for publication in inner.publications.values_mut() {
            if publication.application_id == input.application_id {
                publication.active = false;
                publication.api_enabled = false;
            }
        }
        Ok(())
    }
}
