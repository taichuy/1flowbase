use super::PortableTemplatePackage;
use std::collections::BTreeMap;

/// Every source key in the installer map has exactly one logical owner. Shared flow
/// snapshots and plugin installations may repeat the same key only for that owner.
pub(super) fn validate_identity_namespace(package: &PortableTemplatePackage) -> Vec<String> {
    let mut claims = BTreeMap::new();
    let mut failures = Vec::new();
    let mut claim = |key: String, owner: String| {
        if key.is_empty() {
            failures.push(format!("portable_template_empty_identity:{owner}"));
        } else if let Some(previous) = claims.insert(key.clone(), owner.clone()) {
            if previous != owner {
                failures.push(format!("portable_template_identity_collision:{key}"));
            }
        }
    };
    for model in &package.data_models {
        claim(model.id.to_string(), format!("model:{}", model.id));
        for field in &model.fields {
            claim(
                field.id.to_string(),
                format!("field:{}:{}", model.id, field.id),
            );
        }
    }
    for app in &package.applications {
        claim(app.id.to_string(), format!("application:{}", app.id));
        for document in std::iter::once(&app.flow_document)
            .chain(app.published.iter().map(|p| &p.flow_document))
        {
            if let Some(id) = document
                .pointer("/meta/flowId")
                .and_then(serde_json::Value::as_str)
            {
                claim(id.to_owned(), format!("flow:{}", app.id));
            }
        }
    }
    for page in &package.pages {
        claim(page.id.to_string(), format!("page:{}", page.id));
        for tab in &page.tabs {
            claim(tab.id.to_string(), format!("tab:{}:{}", page.id, tab.id));
            claim(
                tab.document_root_uid.clone(),
                format!("document:{}:{}", page.id, tab.id),
            );
            for block in &tab.blocks {
                let owner = format!("{}:{}:{}", page.id, tab.id, block.block_id);
                claim(block.block_id.clone(), format!("block:{owner}"));
                claim(block.code_ref.clone(), format!("code:{owner}"));
                let descriptor = &block.runtime_descriptor;
                if let Some(id) = descriptor
                    .pointer("/catalog/installationId")
                    .and_then(serde_json::Value::as_str)
                {
                    let plugin = descriptor
                        .pointer("/contribution/pluginId")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    let version = descriptor
                        .pointer("/contribution/pluginVersion")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    claim(id.to_owned(), format!("installation:{plugin}:{version}"));
                }
            }
        }
    }
    failures
}
