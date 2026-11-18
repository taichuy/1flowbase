use super::*;
use async_trait::async_trait;
use domain::{
    ConsoleOperationPolicy, RecordPermissionGrant, RecordPermissionResourceKey,
    RoleConsoleGroupPolicy, RoleConsolePolicy,
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone, Default)]
struct Memory(Arc<Mutex<State>>);
#[derive(Default)]
struct State {
    policies: std::collections::BTreeMap<String, RecordPermissionPolicy>,
    roles: Vec<RecordPermissionRole>,
    effective_roles: Vec<Uuid>,
    console_operations: Vec<String>,
    writes: usize,
}
fn identity(key: &RecordPermissionKey) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        key.workspace_id,
        key.resource.publisher_namespace,
        key.resource.plugin_code,
        key.resource.resource_code,
        key.record_id
    )
}
#[async_trait]
impl RecordPermissionRepository for Memory {
    async fn get_record_permissions(
        &self,
        key: &RecordPermissionKey,
    ) -> Result<Option<RecordPermissionPolicy>> {
        Ok(self.0.lock().unwrap().policies.get(&identity(key)).cloned())
    }
    async fn replace_record_permissions(
        &self,
        key: &RecordPermissionKey,
        input: &ReplaceRecordPermissions,
        _actor: Uuid,
    ) -> Result<RecordPermissionPolicy> {
        let mut state = self.0.lock().unwrap();
        let previous = state
            .policies
            .get(&identity(key))
            .cloned()
            .unwrap_or_default();
        if input
            .expected_revision
            .is_some_and(|revision| revision != previous.revision)
        {
            return Err(ControlPlaneError::Conflict("record_permission_revision").into());
        }
        let policy = RecordPermissionPolicy {
            mode: input.mode,
            grants: input.grants.clone(),
            revision: previous.revision + 1,
        };
        state.policies.insert(identity(key), policy.clone());
        state.writes += 1;
        Ok(policy)
    }
    async fn record_permission_effective_role_ids(
        &self,
        _actor: &ActorContext,
    ) -> Result<Vec<Uuid>> {
        Ok(self.0.lock().unwrap().effective_roles.clone())
    }
    async fn list_record_permission_roles(
        &self,
        _workspace: Uuid,
    ) -> Result<Vec<RecordPermissionRole>> {
        Ok(self.0.lock().unwrap().roles.clone())
    }
}
#[async_trait]
impl RoleConsolePolicyReader for Memory {
    async fn load_role_console_policies_for_user(
        &self,
        _actor: &ActorContext,
    ) -> Result<Vec<RoleConsolePolicy>> {
        let state = self.0.lock().unwrap();
        Ok(vec![RoleConsolePolicy::new(
            Uuid::new_v4(),
            vec![RoleConsoleGroupPolicy::custom(
                ConsolePolicyGroup::settings_feature(
                    access_control::SYSTEM_ROLES_SETTINGS_FEATURE_ID,
                )
                .unwrap(),
                state
                    .console_operations
                    .iter()
                    .map(|operation| {
                        ConsoleOperationPolicy::simple(
                            ConsoleOperationId::try_from(operation.as_str()).unwrap(),
                            true,
                        )
                    })
                    .collect(),
            )],
        )])
    }
}
fn fixture() -> (
    RecordPermissionService<Memory>,
    ActorContext,
    RecordPermissionKey,
    Uuid,
) {
    let role = Uuid::new_v4();
    let workspace = Uuid::new_v4();
    let actor = ActorContext::scoped(
        Uuid::new_v4(),
        workspace,
        "operator",
        vec!["ssh.hosts.update".to_string()],
    );
    let key = RecordPermissionKey {
        resource: RecordPermissionResourceKey {
            publisher_namespace: "acme".into(),
            plugin_code: "ssh".into(),
            resource_code: "hosts".into(),
        },
        record_id: "host-a".into(),
        workspace_id: workspace,
    };
    let memory = Memory::default();
    {
        let mut state = memory.0.lock().unwrap();
        state.roles.push(RecordPermissionRole {
            role_id: role,
            code: "operator".into(),
            name: "Operator".into(),
        });
        state.effective_roles = vec![role];
    }
    (RecordPermissionService::new(memory), actor, key, role)
}
fn restricted(role: Uuid) -> ReplaceRecordPermissions {
    ReplaceRecordPermissions {
        mode: RecordPermissionMode::Restricted,
        grants: vec![RecordPermissionGrant {
            role_id: role,
            actions: vec!["open".into()],
        }],
        expected_revision: Some(0),
    }
}
fn permit_management(service: &RecordPermissionService<Memory>) {
    service.repository.0.lock().unwrap().console_operations = vec![
        RECORD_PERMISSIONS_VIEW_OPERATION_ID.into(),
        RECORD_PERMISSIONS_REPLACE_OPERATION_ID.into(),
    ];
}

#[tokio::test]
async fn unconfigured_inherits_coarse_authorization_and_row_grant_cannot_expand_it() {
    let (service, actor, key, role) = fixture();
    assert!(service.authorize(&actor, &key, "open", true).await.is_ok());
    assert!(service
        .authorize(&actor, &key, "open", false)
        .await
        .is_err());
    permit_management(&service);
    service
        .replace(&actor, &key, &restricted(role), &["open".into()])
        .await
        .unwrap();
    assert!(service.authorize(&actor, &key, "open", true).await.is_ok());
    assert!(service
        .authorize(&actor, &key, "open", false)
        .await
        .is_err());
    assert!(service
        .authorize(&actor, &key, "update", true)
        .await
        .is_err());
}

#[tokio::test]
async fn effective_role_revocation_and_empty_header_take_effect_on_next_call() {
    let (service, actor, key, role) = fixture();
    permit_management(&service);
    service
        .replace(&actor, &key, &restricted(role), &["open".into()])
        .await
        .unwrap();
    assert!(service.authorize(&actor, &key, "open", true).await.is_ok());
    service.repository.0.lock().unwrap().effective_roles.clear();
    assert!(service.authorize(&actor, &key, "open", true).await.is_err());
    let empty = ReplaceRecordPermissions {
        mode: RecordPermissionMode::Restricted,
        grants: vec![],
        expected_revision: Some(1),
    };
    service
        .replace(&actor, &key, &empty, &["open".into()])
        .await
        .unwrap();
    service.repository.0.lock().unwrap().effective_roles = vec![role];
    assert!(service.authorize(&actor, &key, "open", true).await.is_err());
    let inherit = ReplaceRecordPermissions {
        mode: RecordPermissionMode::Inherit,
        grants: vec![],
        expected_revision: Some(2),
    };
    service
        .replace(&actor, &key, &inherit, &["open".into()])
        .await
        .unwrap();
    assert!(service.authorize(&actor, &key, "open", true).await.is_ok());
}

#[tokio::test]
async fn host_update_permission_is_not_policy_management_and_invalid_writes_have_no_side_effect() {
    let (service, actor, key, role) = fixture();
    assert!(service
        .replace(&actor, &key, &restricted(role), &["open".into()])
        .await
        .is_err());
    assert!(service.get(&actor, &key).await.is_err());
    assert!(service.roles(&actor).await.is_err());
    assert!(!service.can_manage_permissions(&actor).await.unwrap());
    permit_management(&service);
    let mut invalid = restricted(Uuid::new_v4());
    assert!(service
        .replace(&actor, &key, &invalid, &["open".into()])
        .await
        .is_err());
    invalid = restricted(role);
    invalid.grants[0].actions.push("undeclared".into());
    assert!(service
        .replace(&actor, &key, &invalid, &["open".into()])
        .await
        .is_err());
    invalid = restricted(role);
    invalid.grants.push(invalid.grants[0].clone());
    assert!(service
        .replace(&actor, &key, &invalid, &["open".into()])
        .await
        .is_err());
    invalid = restricted(role);
    invalid.mode = RecordPermissionMode::Inherit;
    assert!(service
        .replace(&actor, &key, &invalid, &["open".into()])
        .await
        .is_err());
    assert_eq!(service.repository.0.lock().unwrap().writes, 0);
    service
        .replace(&actor, &key, &restricted(role), &["open".into()])
        .await
        .unwrap();
    assert!(service
        .replace(&actor, &key, &restricted(role), &["open".into()])
        .await
        .is_err());
    assert_eq!(service.get(&actor, &key).await.unwrap().revision, 1);
}

#[tokio::test]
async fn root_comes_from_actor_and_cannot_override_caller_workspace() {
    let (service, mut actor, key, _) = fixture();
    service.repository.0.lock().unwrap().policies.insert(
        identity(&key),
        RecordPermissionPolicy {
            mode: RecordPermissionMode::Restricted,
            grants: vec![],
            revision: 1,
        },
    );
    actor.effective_display_role = "root".into();
    actor.permissions.insert("root".into());
    assert!(service.authorize(&actor, &key, "open", true).await.is_err());
    actor.is_root = true;
    assert!(service.authorize(&actor, &key, "open", false).await.is_ok());
    assert!(service.can_manage_permissions(&actor).await.unwrap());
    let mut other = key.clone();
    other.workspace_id = Uuid::new_v4();
    assert!(service
        .authorize(&actor, &other, "open", true)
        .await
        .is_err());
    assert!(service.get(&actor, &other).await.is_err());
}

#[tokio::test]
async fn capabilities_intersect_host_actions_and_record_roles_without_cross_plugin_policy_leak() {
    let (service, actor, key, role) = fixture();
    permit_management(&service);
    service
        .replace(&actor, &key, &restricted(role), &["open".into()])
        .await
        .unwrap();
    let caps = service
        .capabilities(
            &actor,
            &key,
            &["view".into(), "open".into(), "delete".into()],
        )
        .await
        .unwrap();
    assert_eq!(caps.allowed_actions, vec!["open"]);
    assert!(caps.can_manage_permissions);
    assert!(service
        .capabilities(&actor, &key, &["view".into()])
        .await
        .unwrap()
        .allowed_actions
        .is_empty());
    let mut other = key.clone();
    other.resource.publisher_namespace = "other".into();
    assert!(service
        .authorize(&actor, &other, "view", true)
        .await
        .is_ok());
    other = key.clone();
    other.resource.plugin_code = "other".into();
    assert!(service
        .authorize(&actor, &other, "view", true)
        .await
        .is_ok());
    other = key.clone();
    other.resource.resource_code = "other".into();
    assert!(service
        .authorize(&actor, &other, "view", true)
        .await
        .is_ok());
}

#[tokio::test]
async fn editor_capability_requires_view_and_replace_but_each_api_keeps_its_own_operation() {
    let (service, actor, key, role) = fixture();
    service.repository.0.lock().unwrap().console_operations =
        vec![RECORD_PERMISSIONS_REPLACE_OPERATION_ID.into()];
    assert!(!service.can_manage_permissions(&actor).await.unwrap());
    assert!(service.get(&actor, &key).await.is_err());
    assert!(service
        .replace(&actor, &key, &restricted(role), &["open".into()])
        .await
        .is_ok());
    service.repository.0.lock().unwrap().console_operations =
        vec![RECORD_PERMISSIONS_VIEW_OPERATION_ID.into()];
    assert!(!service.can_manage_permissions(&actor).await.unwrap());
    assert!(service.get(&actor, &key).await.is_ok());
    assert!(service
        .replace(&actor, &key, &restricted(role), &["open".into()])
        .await
        .is_err());
    permit_management(&service);
    assert!(service.can_manage_permissions(&actor).await.unwrap());
}
