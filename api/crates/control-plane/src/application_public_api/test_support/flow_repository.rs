use super::*;

#[async_trait]
impl FlowRepository for ApplicationPublicApiTestRepository {
    async fn get_or_create_editor_state(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<domain::FlowEditorState> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        inner.editor_state_read_count += 1;
        let application = inner
            .applications
            .get(&application_id)
            .filter(|application| application.workspace_id == workspace_id)
            .cloned()
            .ok_or(ControlPlaneError::NotFound("application"))?;
        if let Some(state) = inner.editor_states.get(&application_id).cloned() {
            return Ok(state);
        }

        inner.next_flow_ordinal += 1;
        inner.next_flow_version_sequence += 1;
        let flow_id =
            deterministic_test_id(0x11111111111111110000000000000000, inner.next_flow_ordinal);
        let draft_id =
            deterministic_test_id(0x22222222222222220000000000000000, inner.next_flow_ordinal);
        let version_id =
            deterministic_test_id(0x33333333333333330000000000000000, inner.next_flow_ordinal);
        let now =
            OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(inner.next_flow_ordinal as i64);
        let document =
            domain::default_flow_document_for_application(application.application_type, flow_id);
        let state = domain::FlowEditorState {
            flow: domain::FlowRecord {
                id: flow_id,
                application_id: application.id,
                created_by: actor_user_id,
                updated_at: now,
            },
            draft: domain::FlowDraftRecord {
                id: draft_id,
                flow_id,
                schema_version: domain::FLOW_SCHEMA_VERSION.to_string(),
                document: document.clone(),
                updated_at: now,
            },
            versions: vec![domain::FlowVersionRecord {
                id: version_id,
                flow_id,
                sequence: inner.next_flow_version_sequence,
                trigger: domain::FlowVersionTrigger::Autosave,
                change_kind: domain::FlowChangeKind::Logical,
                summary: "初始化默认草稿".to_string(),
                summary_is_custom: false,
                is_protected: false,
                document,
                created_at: now,
            }],
            autosave_interval_seconds: domain::FLOW_AUTOSAVE_INTERVAL_SECONDS,
        };
        inner.editor_states.insert(application_id, state.clone());
        Ok(state)
    }

    async fn save_draft(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        document: serde_json::Value,
        change_kind: domain::FlowChangeKind,
        summary: &str,
    ) -> Result<domain::FlowEditorState> {
        let mut state = self
            .get_or_create_editor_state(workspace_id, application_id, actor_user_id)
            .await?;
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        inner.next_flow_ordinal += 1;
        let now =
            OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(inner.next_flow_ordinal as i64);
        state.draft.document = document.clone();
        state.draft.schema_version = document
            .get("schemaVersion")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(domain::FLOW_SCHEMA_VERSION)
            .to_string();
        state.draft.updated_at = now;
        state.flow.updated_at = now;

        if matches!(change_kind, domain::FlowChangeKind::Logical) {
            inner.next_flow_version_sequence += 1;
            state.versions.push(domain::FlowVersionRecord {
                id: deterministic_test_id(
                    0x33333333333333330000000000000000,
                    inner.next_flow_version_sequence as u128,
                ),
                flow_id: state.flow.id,
                sequence: inner.next_flow_version_sequence,
                trigger: domain::FlowVersionTrigger::Autosave,
                change_kind,
                summary: summary.to_string(),
                summary_is_custom: false,
                is_protected: false,
                document,
                created_at: now,
            });
        }
        inner.editor_states.insert(application_id, state.clone());
        Ok(state)
    }

    async fn restore_version(
        &self,
        _workspace_id: Uuid,
        _application_id: Uuid,
        _actor_user_id: Uuid,
        _version_id: Uuid,
    ) -> Result<domain::FlowEditorState> {
        anyhow::bail!("restore_version not implemented")
    }

    async fn update_version_metadata(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        version_id: Uuid,
        summary: Option<String>,
        summary_is_custom: Option<bool>,
        is_protected: Option<bool>,
    ) -> Result<domain::FlowEditorState> {
        let mut state = self
            .get_or_create_editor_state(workspace_id, application_id, actor_user_id)
            .await?;
        let version = state
            .versions
            .iter_mut()
            .find(|version| version.id == version_id)
            .ok_or(ControlPlaneError::NotFound("flow_version"))?;
        if let Some(summary) = summary {
            version.summary = summary;
        }
        if let Some(summary_is_custom) = summary_is_custom {
            version.summary_is_custom = summary_is_custom;
        }
        if let Some(is_protected) = is_protected {
            version.is_protected = is_protected;
        }
        self.inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .editor_states
            .insert(application_id, state.clone());
        Ok(state)
    }
}
