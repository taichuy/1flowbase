use std::{collections::HashSet, sync::Arc};

use control_plane::{
    navigation_cache::{role_policy_variant, NavigationCache, NavigationCacheDomain},
    ports::RoleConsolePolicyReader,
};
use domain::ActorContext;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error_response::ApiError;
use storage_durable_postgres::MainDurableStore;

#[derive(Clone)]
pub struct ConsolePolicyProjector {
    bootstrap_workspace_id: Uuid,
    operations: Arc<access_control::ConsoleOperationRegistry>,
    features: Arc<access_control::SettingsFeatureRegistry>,
    reader: Arc<dyn RoleConsolePolicyReader>,
    cache: NavigationCache,
    inventory_key: String,
}

#[derive(Serialize, Deserialize)]
struct Projection {
    permissions: HashSet<String>,
    policy_fingerprint: String,
}

fn digest(value: impl Serialize) -> String {
    let bytes = serde_json::to_vec(&value).expect("serializable console policy input");
    format!("{:x}", base16ct::HexDisplay(&Sha256::digest(bytes)[..16]))
}

pub fn policy_fingerprint(actor: &ActorContext) -> String {
    actor.console_policy_fingerprint.clone().unwrap_or_else(|| {
        let permissions = actor
            .permissions
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        digest((&actor.effective_display_role, actor.is_root, permissions))
    })
}

impl ConsolePolicyProjector {
    pub fn new(
        bootstrap_workspace_id: Uuid,
        operations: Arc<access_control::ConsoleOperationRegistry>,
        features: Arc<access_control::SettingsFeatureRegistry>,
        reader: Arc<dyn RoleConsolePolicyReader>,
        cache: NavigationCache,
    ) -> Self {
        let inventory_key = digest((operations.inventory(), features.inventory()));
        Self {
            bootstrap_workspace_id,
            operations,
            features,
            reader,
            cache,
            inventory_key,
        }
    }

    /// Evaluate only the requested codes against the already authenticated projection.
    pub fn evaluate_access(
        &self,
        actor: &ActorContext,
        codes: Vec<String>,
    ) -> std::collections::BTreeMap<String, bool> {
        let outside_bootstrap = actor.current_workspace_id != self.bootstrap_workspace_id;
        let restricted: HashSet<&str> = if outside_bootstrap {
            self.operations
                .inventory()
                .operations
                .iter()
                .filter(|operation| operation.root_workspace_only)
                .map(|operation| operation.operation_id.as_str())
                .chain(
                    self.features
                        .inventory()
                        .features
                        .iter()
                        .filter(|feature| feature.root_workspace_only)
                        .map(|feature| feature.permission_code.as_str()),
                )
                .collect()
        } else {
            HashSet::new()
        };
        let mut allowed = std::collections::BTreeMap::new();
        for code in codes.into_iter().filter(|code| !code.trim().is_empty()) {
            if !allowed.contains_key(&code) {
                let grant = !restricted.contains(code.as_str())
                    && (actor.is_root || actor.permissions.contains(&code));
                allowed.insert(code, grant);
            }
        }
        allowed
    }

    pub async fn invalidate(&self, workspace_id: Uuid) {
        self.cache
            .invalidate(NavigationCacheDomain::ConsoleRoutes, workspace_id)
            .await;
    }

    /// Only for actors whose workspace and active role were validated against the durable store.
    pub async fn project(
        &self,
        store: &MainDurableStore,
        actor: &mut ActorContext,
    ) -> Result<(), ApiError> {
        let outside_bootstrap = actor.current_workspace_id != self.bootstrap_workspace_id;
        let variant = format!(
            "role-projection:{}:{}:{}:{}",
            self.inventory_key,
            role_policy_variant(actor),
            actor.is_root,
            outside_bootstrap
        );
        let result = self
            .cache
            .load(
                NavigationCacheDomain::ConsoleRoutes,
                actor.current_workspace_id,
                &variant,
                async {
                    let mut actor = actor.clone();
                    actor.permissions = store.load_actor_legacy_permissions(&actor).await?;
                    self.project_existing(&mut actor).await?;
                    Ok(Projection {
                        policy_fingerprint: policy_fingerprint(&actor),
                        permissions: actor.permissions,
                    })
                },
            )
            .await?;
        actor.permissions = result.permissions;
        actor.console_policy_fingerprint = Some(result.policy_fingerprint);
        Ok(())
    }

    /// In-process delegation and OAuth may already supply legacy permissions.
    pub async fn project_existing(&self, actor: &mut ActorContext) -> anyhow::Result<()> {
        let policies = if actor.is_root {
            Vec::new()
        } else {
            self.reader
                .load_role_console_policies_for_user(actor)
                .await?
        };
        let outside_bootstrap = actor.current_workspace_id != self.bootstrap_workspace_id;
        apply_projection(
            actor,
            outside_bootstrap,
            self.operations.inventory(),
            &self.features.inventory().features,
            &policies,
        );
        let permissions = actor
            .permissions
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        actor.console_policy_fingerprint = Some(digest((
            &self.inventory_key,
            &actor.effective_display_role,
            actor.is_root,
            outside_bootstrap,
            permissions,
            policies,
        )));
        Ok(())
    }
}

fn apply_projection(
    actor: &mut ActorContext,
    outside_bootstrap: bool,
    inventory: &access_control::ConsoleOperationCompiledInventory,
    features: &[access_control::SettingsFeatureInventoryEntry],
    policies: &[domain::RoleConsolePolicy],
) {
    if actor.is_root {
        if outside_bootstrap {
            remove_platform_permissions(actor, inventory, features);
        } else {
            // Clients must use the explicit projection, not the root role alone,
            // to distinguish platform operations available in this session.
            actor.permissions.extend(
                inventory
                    .operations
                    .iter()
                    .filter(|operation| {
                        operation.root_workspace_only
                            && operation.lifecycle
                                == access_control::SettingsFeatureLifecycle::Active
                    })
                    .map(|operation| operation.operation_id.clone()),
            );
        }
        return;
    }
    // Project the same effective operations consumed by the invocation boundary. Legacy
    // role grants cannot retain stale operation IDs after a group or operation is revoked.
    actor
        .permissions
        .retain(|permission| !permission.starts_with("settings_feature.access."));
    actor.permissions.remove("frontstage.page.design");
    for operation in &inventory.operations {
        actor.permissions.remove(&operation.operation_id);
    }
    let permissions = console_operation_permissions(inventory, policies);
    if inventory.operations.iter().any(|operation| {
        permissions.contains(&operation.operation_id)
            && operation
                .authorization_profile_id
                .starts_with("frontstage.")
            && operation
                .routes
                .iter()
                .any(|route| matches!(route.method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE"))
    }) {
        actor
            .permissions
            .insert("frontstage.page.design".to_string());
    }
    actor.permissions.extend(permissions);
    actor
        .permissions
        .extend(settings_feature_navigation_permissions(
            inventory, policies, features,
        ));
    if outside_bootstrap {
        remove_platform_permissions(actor, inventory, features);
    }
}

fn remove_platform_permissions(
    actor: &mut ActorContext,
    inventory: &access_control::ConsoleOperationCompiledInventory,
    features: &[access_control::SettingsFeatureInventoryEntry],
) {
    for operation in inventory
        .operations
        .iter()
        .filter(|op| op.root_workspace_only)
    {
        actor.permissions.remove(&operation.operation_id);
    }
    for feature in features
        .iter()
        .filter(|feature| feature.root_workspace_only)
    {
        actor.permissions.remove(&feature.permission_code);
    }
}

/// A settings surface is discoverable if at least one active operation is granted.
/// Row visibility is not a row-data grant: the request's resource/workspace checks still apply.
pub fn settings_feature_navigation_permissions(
    inventory: &access_control::ConsoleOperationCompiledInventory,
    policies: &[domain::RoleConsolePolicy],
    settings_features: &[access_control::SettingsFeatureInventoryEntry],
) -> std::collections::BTreeSet<String> {
    let mut permissions = std::collections::BTreeSet::new();
    for operation in &inventory.operations {
        if operation.lifecycle != access_control::SettingsFeatureLifecycle::Active {
            continue;
        }
        let (group, feature_ids) = match &operation.policy_group {
            access_control::ConsolePolicyGroup::SettingsFeature(id) => (
                domain::ConsolePolicyGroup::settings_feature(id),
                vec![id.as_str()],
            ),
            access_control::ConsolePolicyGroup::Plugin(id) => (
                domain::ConsolePolicyGroup::plugin(id),
                settings_features
                    .iter()
                    .filter(|feature| feature.owner == operation.owner)
                    .map(|feature| feature.feature_id.as_str())
                    .collect(),
            ),
            access_control::ConsolePolicyGroup::Other(_) => continue,
        };
        let (Ok(group), Ok(id)) = (
            group,
            domain::ConsoleOperationId::try_from(operation.operation_id.as_str()),
        ) else {
            continue;
        };
        let granted = match operation.authorization {
            access_control::ConsoleAuthorization::Simple => {
                domain::effective_console_simple_operation(policies, &group, &id)
            }
            access_control::ConsoleAuthorization::ResourceAction { .. } => {
                domain::effective_console_row_scope(policies, &group, &id)
                    != domain::ConsoleOperationRowScope::Disabled
            }
            access_control::ConsoleAuthorization::Authenticated => false,
        };
        if granted {
            permissions.extend(
                feature_ids
                    .into_iter()
                    .map(|id| format!("settings_feature.access.{id}")),
            );
        }
    }
    permissions
}

fn console_operation_permissions(
    inventory: &access_control::ConsoleOperationCompiledInventory,
    policies: &[domain::RoleConsolePolicy],
) -> std::collections::BTreeSet<String> {
    inventory
        .operations
        .iter()
        .filter(|operation| operation.lifecycle == access_control::SettingsFeatureLifecycle::Active)
        .filter(|operation| {
            let group = match &operation.policy_group {
                access_control::ConsolePolicyGroup::SettingsFeature(id) => {
                    domain::ConsolePolicyGroup::settings_feature(id)
                }
                access_control::ConsolePolicyGroup::Plugin(id) => {
                    domain::ConsolePolicyGroup::plugin(id)
                }
                access_control::ConsolePolicyGroup::Other(id) => {
                    domain::ConsolePolicyGroup::other(id)
                }
            };
            let (Ok(group), Ok(id)) = (
                group,
                domain::ConsoleOperationId::try_from(operation.operation_id.as_str()),
            ) else {
                return false;
            };
            match operation.authorization {
                access_control::ConsoleAuthorization::Simple => {
                    domain::effective_console_simple_operation(policies, &group, &id)
                }
                access_control::ConsoleAuthorization::ResourceAction { .. } => {
                    domain::effective_console_row_scope(policies, &group, &id)
                        != domain::ConsoleOperationRowScope::Disabled
                }
                access_control::ConsoleAuthorization::Authenticated => false,
            }
        })
        .map(|operation| operation.operation_id.clone())
        .collect()
}
