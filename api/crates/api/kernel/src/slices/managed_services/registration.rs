use access_control::*;
use plugin_framework::{ManagedServiceManifest, PluginSettingsPageManifest};

#[derive(Debug, Clone)]
pub struct ManagedServiceRegistration {
    pub shared_resource: Option<control_plane::plugin_management::SharedResourceContract>,
    pub installation_id: uuid::Uuid,
    pub publisher_namespace: String,
    pub data_models: Vec<extension_contracts::PluginDataModelContribution>,
    pub plugin_code: String,
    pub plugin_version: String,
    pub display_name: String,
    pub description: String,
    pub declaration: ManagedServiceManifest,
    pub pages: Vec<PluginSettingsPageManifest>,
}
impl ManagedServiceRegistration {
    pub fn resource_owner(&self) -> (&str, &str, &str) {
        self.shared_resource
            .as_ref()
            .map(|c| {
                (
                    c.owner.publisher_namespace.as_str(),
                    c.owner.plugin_code.as_str(),
                    c.schema_version.as_str(),
                )
            })
            .unwrap_or((
                &self.publisher_namespace,
                &self.plugin_code,
                &self.plugin_version,
            ))
    }

    pub fn owner(&self) -> SettingsFeatureOwner {
        SettingsFeatureOwner {
            kind: SettingsFeatureOwnerKind::ManagedService,
            owner_id: self.plugin_code.clone(),
            version: self.plugin_version.clone(),
        }
    }
    pub fn feature(&self) -> Option<SettingsFeatureRegistration> {
        if self.pages.is_empty() {
            return None;
        }
        let f = self.declaration.feature.as_ref()?;
        Some(SettingsFeatureRegistration {
            feature_id: f.feature_id.clone(),
            owner: self.owner(),
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: f.route_id.clone(),
                surface_key: format!("managed-service:{}", self.plugin_code),
                path: f.path.clone(),
                label_key: f.label.clone(),
                description_key: f.description.clone(),
                order: 1000,
            },
            api_routes: self
                .declaration
                .operations
                .iter()
                .map(|op| SettingsApiRoute {
                    method: op.method.clone(),
                    path: op.path.clone(),
                })
                .collect(),
        })
    }
    pub fn locale_catalog(&self) -> ConsoleLocaleCatalogContribution {
        if let Some(feature) = self.feature() {
            // Settings surfaces may have different captions; their permissions still belong to
            // the one plugin group.
            let mut contribution = self.plugin_locale_catalog();
            for text in [
                feature.console_surface.label_key,
                feature.console_surface.description_key,
            ] {
                if !contribution
                    .texts
                    .iter()
                    .any(|existing| existing.reference == text)
                {
                    contribution.texts.push(ConsoleLocaleText {
                        reference: text.clone(),
                        en_us: text.clone(),
                        zh_hans: text,
                    });
                }
            }
            return contribution;
        }
        self.plugin_locale_catalog()
    }
    fn plugin_locale_catalog(&self) -> ConsoleLocaleCatalogContribution {
        let texts = [&self.display_name, &self.description]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|text| ConsoleLocaleText {
                reference: text.clone(),
                en_us: text.clone(),
                zh_hans: text.clone(),
            })
            .collect();
        ConsoleLocaleCatalogContribution {
            owner: self.owner(),
            lifecycle: SettingsFeatureLifecycle::Active,
            texts,
            policy_groups: Vec::new(),
            plugin_groups: vec![ConsoleOtherPolicyGroupDisplay {
                group_id: self.plugin_code.clone(),
                label_ref: self.display_name.clone(),
                description_ref: self.description.clone(),
            }],
        }
    }
    pub fn operations(&self) -> Vec<ConsoleOperationRegistration> {
        self.declaration
            .operations
            .iter()
            .enumerate()
            .map(|(index, op)| ConsoleOperationRegistration {
                operation_id: op.interface_id.clone(),
                authorization_profile_id: Some(op.interface_id.clone()),
                owner: self.owner(),
                lifecycle: SettingsFeatureLifecycle::Active,
                policy_group: ConsolePolicyGroup::Plugin(self.plugin_code.clone()),
                order: index as i32,
                routes: vec![ConsoleRouteBinding {
                    method: op.method.clone(),
                    path: op.path.clone(),
                }],
                authorization: ConsoleAuthorization::Simple,
            })
            .collect()
    }
    pub fn metadata(&self) -> Vec<ConsoleInterfaceRegistration> {
        self.declaration
            .operations
            .iter()
            .flat_map(|op| {
                let mut rows = vec![ConsoleInterfaceRegistration {
                interface_id: op.interface_id.clone(),
                route: ConsoleRouteBinding {
                    method: op.method.clone(),
                    path: op.path.clone(),
                },
                summary: op.summary.clone(),
                description: op.description.clone(),
            }];
                if op.stream.is_some() {
                    rows.push(ConsoleInterfaceRegistration {
                        interface_id: format!("{}.stream-upgrade",op.interface_id),
                        route: ConsoleRouteBinding {method:"GET".into(),path:op.path.clone()},
                        summary: format!("Open {} stream", op.summary),
                        description: "Upgrades a single-use session and Origin bound ticket; execution remains authorized by the declared POST operation.".into(),
                    });
                }
                rows
            })
            .collect()
    }
}
