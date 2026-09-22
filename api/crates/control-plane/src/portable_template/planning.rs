use super::references::{route_references, string_values};
use super::*;
use std::collections::BTreeSet;

pub fn validate_portable_template(package: &PortableTemplatePackage) -> Vec<String> {
    let mut failures = super::identity::validate_identity_namespace(package);
    if package.schema_version != PORTABLE_TEMPLATE_SCHEMA_VERSION {
        failures.push("portable_template_schema_version".into());
    }
    if package.pages.len() + package.applications.len() + package.data_models.len() == 0 {
        failures.push("portable_template_empty".into());
    }
    if package.pages.len() > 1000
        || package.applications.len() > 100
        || package.data_models.len() > 1000
    {
        failures.push("portable_template_limit".into());
    }
    let mut ids = BTreeSet::new();
    for id in package
        .pages
        .iter()
        .map(|p| p.id)
        .chain(package.applications.iter().map(|a| a.id))
        .chain(package.data_models.iter().map(|m| m.id))
    {
        if id.is_nil() || !ids.insert(id) {
            failures.push(format!("portable_template_duplicate_or_nil_id:{id}"));
        }
    }
    let mut model_codes = BTreeSet::new();
    for m in &package.data_models {
        if !m.builtin && m.template_provider != domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER {
            failures.push(format!(
                "portable_template_unsupported_model_template_provider:{}",
                m.template_provider
            ));
        }

        if !model_codes.insert(&m.code) || m.code.trim().is_empty() {
            failures.push(format!("portable_template_model_code:{}", m.code));
        }
        let mut fields = BTreeSet::new();
        for f in &m.fields {
            if !fields.insert(&f.code) || !ids.insert(f.id) {
                failures.push(format!(
                    "portable_template_duplicate_field:{}:{}",
                    m.code, f.code
                ));
            }
            if !f.is_system && !m.builtin {
                if let Some(id) = f.relation_target_model_id {
                    if !package.data_models.iter().any(|m| m.id == id) {
                        failures.push(format!("portable_template_unresolved_relation:{id}"));
                    }
                }
                inspect_credentials(&f.default_value.clone().unwrap_or_default(), &mut failures);
            }
        }
    }
    let mut slugs = BTreeSet::new();
    let mut global_block_ids = BTreeSet::new();
    for page in &package.pages {
        if let Err(error) =
            crate::frontstage::root_slug_for(page.parent_id, page.placement, page.slug.clone())
        {
            failures.push(format!("portable_template_page_route:{}:{error}", page.id));
        }
        if let Some(slug) = &page.slug {
            if !slugs.insert(slug) {
                failures.push(format!("portable_template_duplicate_page_slug:{slug}"));
            }
        }
        let mut visited = BTreeSet::new();
        let mut parent = Some(page.id);
        while let Some(id) = parent {
            if !visited.insert(id) {
                failures.push(format!("portable_template_page_cycle:{}", page.id));
                break;
            }
            match package.pages.iter().find(|p| p.id == id) {
                Some(p) => parent = p.parent_id,
                None => {
                    failures.push(format!("portable_template_missing_page_parent:{id}"));
                    break;
                }
            }
        }
        if page.kind == domain::FrontstagePageKind::Page
            && page.tabs.iter().filter(|t| t.is_default).count() != 1
        {
            failures.push(format!("portable_template_default_tab:{}", page.id));
        }
        if page.kind == domain::FrontstagePageKind::Group && !page.tabs.is_empty() {
            failures.push(format!("portable_template_group_tabs:{}", page.id));
        }
        for rule in &page.visibility_rules {
            if rule.role_code == "root"
                || rule.visibility == domain::frontstage::FrontstagePageVisibility::Hidden
            {
                failures.push(format!(
                    "portable_template_unsupported_visibility:{}",
                    rule.role_code
                ));
            }
            if rule.role_code.trim().is_empty()
                || rule
                    .tab_id
                    .is_some_and(|id| !page.tabs.iter().any(|tab| tab.id == id))
            {
                failures.push(format!(
                    "portable_template_invalid_visibility_rule:{}",
                    page.id
                ));
            }
        }
        for tab in &page.tabs {
            if !ids.insert(tab.id) {
                failures.push(format!("portable_template_duplicate_tab:{}", tab.id));
            }
            let mut block_ids = BTreeSet::new();
            for block in &tab.blocks {
                if !block_ids.insert(&block.block_id) || !global_block_ids.insert(&block.block_id) {
                    failures.push(format!(
                        "portable_template_duplicate_block:{}",
                        block.block_id
                    ));
                }
                let mut visited = BTreeSet::new();
                let mut parent = Some(block.block_id.as_str());
                while let Some(id) = parent {
                    if !visited.insert(id) {
                        failures.push(format!("portable_template_block_cycle:{id}"));
                        break;
                    }
                    match tab.blocks.iter().find(|b| b.block_id == id) {
                        Some(b) => parent = b.parent_block_id.as_deref(),
                        None => {
                            failures.push(format!("portable_template_missing_block_parent:{id}"));
                            break;
                        }
                    }
                }
            }
        }
    }
    let mut extension_slugs = BTreeSet::new();
    for app in &package.applications {
        failures.extend(
            app.dependency_issues
                .iter()
                .map(|issue| format!("{issue}:{}", app.id)),
        );
        if let Err(error) = crate::flow::validate_flow_draft_document(&app.flow_document) {
            failures.push(format!("portable_template_flow:{}:{error}", app.id));
        }
        inspect_credentials(&app.flow_document, &mut failures);
        if app.workflow_trigger_type == Some(domain::WorkflowTriggerType::Schedule)
            && app.schedule.is_none()
        {
            failures.push(format!("portable_template_missing_schedule:{}", app.id));
        }
        let own_slugs: BTreeSet<&str> = app
            .mapping
            .as_ref()
            .and_then(|m| m.extension_slug())
            .into_iter()
            .chain(
                app.published
                    .as_ref()
                    .and_then(|p| p.mapping.extension_slug()),
            )
            .collect();
        for slug in own_slugs {
            if !extension_slugs.insert(slug) {
                failures.push(format!("portable_template_duplicate_extension:{slug}"));
            }
        }
        if let Some(p) = &app.published {
            if let Err(error) = crate::flow::validate_flow_draft_document(&p.flow_document) {
                failures.push(format!(
                    "portable_template_published_flow:{}:{error}",
                    app.id
                ));
            }
            inspect_credentials(&p.flow_document, &mut failures);
        }
    }
    // Recognized typed UUID references must resolve; arbitrary UUID text remains user content.
    fn typed_refs(v: &serde_json::Value, known: &BTreeSet<uuid::Uuid>, failures: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(o) => {
                for (key, value) in o {
                    if matches!(
                        key.as_str(),
                        "data_model_id"
                            | "model_definition_id"
                            | "application_id"
                            | "page_id"
                            | "tab_id"
                            | "field_id"
                    ) {
                        if let Some(id) = value.as_str().and_then(|s| uuid::Uuid::parse_str(s).ok())
                        {
                            if !known.contains(&id) {
                                failures.push(format!(
                                    "portable_template_unresolved_reference:{key}:{id}"
                                ));
                            }
                        }
                    }
                    typed_refs(value, known, failures);
                }
            }
            serde_json::Value::Array(a) => {
                for value in a {
                    typed_refs(value, known, failures);
                }
            }
            _ => {}
        }
    }
    for app in &package.applications {
        typed_refs(&app.flow_document, &ids, &mut failures);
        if let Some(p) = &app.published {
            typed_refs(&p.flow_document, &ids, &mut failures);
        }
    }
    for page in &package.pages {
        for tab in &page.tabs {
            typed_refs(&tab.document_payload, &ids, &mut failures);
            for block in &tab.blocks {
                typed_refs(&block.runtime_descriptor, &ids, &mut failures);
            }
        }
    }
    fn frontend_providers(value: &serde_json::Value, failures: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(provider) = value
                    .pointer("/catalog/providerCode")
                    .and_then(|v| v.as_str())
                {
                    let pinned = value
                        .pointer("/contribution/pluginId")
                        .and_then(|v| v.as_str())
                        .is_some()
                        && value
                            .pointer("/contribution/pluginVersion")
                            .and_then(|v| v.as_str())
                            .is_some();
                    if !pinned
                        && provider != crate::ui_management::DEFAULT_UI_TEMPLATE_PROVIDER_CODE
                    {
                        failures.push(format!(
                            "portable_template_unsupported_frontend_provider:{provider}"
                        ));
                    }
                }
                if let Some(provider) = object.get("provider_code").and_then(|v| v.as_str()) {
                    if object.contains_key("contribution_code")
                        && !object.contains_key("plugin_id")
                        && provider != crate::ui_management::DEFAULT_UI_TEMPLATE_PROVIDER_CODE
                    {
                        failures.push(format!(
                            "portable_template_unsupported_frontend_provider:{provider}"
                        ));
                    }
                }
                for value in object.values() {
                    frontend_providers(value, failures);
                }
            }
            serde_json::Value::Array(items) => {
                for value in items {
                    frontend_providers(value, failures);
                }
            }
            _ => {}
        }
    }
    for page in &package.pages {
        for tab in &page.tabs {
            frontend_providers(&tab.document_payload, &mut failures);
            for block in &tab.blocks {
                frontend_providers(&block.runtime_descriptor, &mut failures);
            }
        }
    }
    let mut strings = Vec::new();
    if let Ok(value) = serde_json::to_value(package) {
        string_values(&value, &mut strings);
    }
    for text in strings {
        for code in route_references(&text, "/api/runtime/models/") {
            if !package.data_models.iter().any(|m| m.code == code) {
                failures.push(format!("portable_template_unresolved_model:{code}"));
            }
        }
        for slug in route_references(&text, "/api/ex/") {
            if !extension_slugs.contains(slug.as_str()) {
                failures.push(format!("portable_template_unresolved_extension:{slug}"));
            }
        }
    }
    for dep in super::export::collect_portable_plugin_dependencies(package, &package.plugins) {
        if !package
            .plugins
            .iter()
            .any(|p| p.plugin_id == dep.plugin_id && p.plugin_version == dep.plugin_version)
        {
            failures.push(format!(
                "portable_template_undeclared_plugin:{}@{}",
                dep.plugin_id, dep.plugin_version
            ));
        }
    }
    failures.sort();
    failures.dedup();
    failures
}
/// Only credential value slots are blocked. Parameter names and API mapping declarations remain intact.
fn inspect_credentials(v: &serde_json::Value, failures: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(o) => {
            for (key, v) in o {
                let normalized = key.to_ascii_lowercase().replace('-', "_");
                if matches!(
                    normalized.as_str(),
                    "password"
                        | "secret"
                        | "api_key"
                        | "access_token"
                        | "refresh_token"
                        | "authorization"
                        | "credentials"
                ) && v
                    .as_str()
                    .is_some_and(|s| !s.is_empty() && !s.starts_with("{{") && !s.starts_with("${"))
                {
                    failures.push(format!("portable_template_inline_credential:{key}"));
                }
                inspect_credentials(v, failures);
            }
        }
        serde_json::Value::Array(a) => {
            for v in a {
                inspect_credentials(v, failures);
            }
        }
        _ => {}
    }
}

pub fn preview_portable_template(
    package: &PortableTemplatePackage,
    target: &PortableTemplatePackage,
) -> PortableTemplatePreview {
    let mut failures = validate_portable_template(package);
    let mut warnings = Vec::new();
    for m in &package.data_models {
        let existing = target.data_models.iter().find(|t| t.code == m.code);
        if m.builtin {
            match existing {
                None => failures.push(format!("portable_template_builtin_missing:{}", m.code)),
                Some(t) if !t.builtin => failures.push(format!(
                    "portable_template_builtin_identity_conflict:{}",
                    m.code
                )),
                Some(t) => {
                    for f in &m.fields {
                        if !t
                            .fields
                            .iter()
                            .any(|tf| tf.code == f.code && tf.field_kind == f.field_kind)
                        {
                            failures.push(format!(
                                "portable_template_builtin_field_missing:{}:{}",
                                m.code, f.code
                            ));
                        }
                    }
                }
            }
        } else if existing.is_some() {
            failures.push(format!("portable_template_model_code_conflict:{}", m.code));
        }
    }
    for page in &package.pages {
        if let Some(slug) = &page.slug {
            if target.pages.iter().any(|p| p.slug.as_ref() == Some(slug)) {
                failures.push(format!("portable_template_page_slug_conflict:{slug}"));
            }
        }
    }
    for app in &package.applications {
        for slug in app
            .mapping
            .as_ref()
            .and_then(|m| m.extension_slug())
            .into_iter()
            .chain(
                app.published
                    .as_ref()
                    .and_then(|p| p.mapping.extension_slug()),
            )
        {
            if target.applications.iter().any(|a| {
                a.mapping.as_ref().and_then(|m| m.extension_slug()) == Some(slug)
                    || a.published
                        .as_ref()
                        .and_then(|p| p.mapping.extension_slug())
                        == Some(slug)
            }) {
                failures.push(format!("portable_template_extension_slug_conflict:{slug}"));
            }
        }
        if app.application_type == domain::ApplicationType::AgentFlow {
            warnings.push(format!(
                "portable_template_provider_credentials_not_included:{}",
                app.id
            ));
        }
    }
    for dep in &package.plugins {
        if !target
            .plugins
            .iter()
            .any(|p| p.plugin_id == dep.plugin_id && p.plugin_version == dep.plugin_version)
        {
            warnings.push(format!(
                "portable_template_plugin_download_required:{}@{}",
                dep.plugin_id, dep.plugin_version
            ));
        }
    }
    for app in &package.applications {
        for extension in app
            .mapping
            .as_ref()
            .and_then(|m| m.extension.as_ref())
            .into_iter()
            .chain(
                app.published
                    .as_ref()
                    .and_then(|p| p.mapping.extension.as_ref()),
            )
        {
            for other in target
                .applications
                .iter()
                .chain(package.applications.iter().filter(|a| a.id != app.id))
            {
                for other_extension in other
                    .mapping
                    .as_ref()
                    .and_then(|m| m.extension.as_ref())
                    .into_iter()
                    .chain(
                        other
                            .published
                            .as_ref()
                            .and_then(|p| p.mapping.extension.as_ref()),
                    )
                {
                    if extension.method==other_extension.method && crate::application_public_api::published_workflow_operation::workflow_route_shapes_conflict(&extension.slug,&other_extension.slug) {
                        failures.push(format!("portable_template_workflow_route_conflict:{}",extension.slug));
                    }
                }
            }
        }
    }
    failures.sort();
    failures.dedup();
    PortableTemplatePreview {
        valid: failures.is_empty(),
        counts: PortableTemplateCounts {
            pages: package.pages.len(),
            applications: package.applications.len(),
            data_models: package.data_models.len(),
        },
        failures,
        warnings,
        dependencies: package.plugins.clone(),
    }
}
