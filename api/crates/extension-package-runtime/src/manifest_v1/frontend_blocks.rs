use std::path::{Component, Path};

use super::*;

const FRONTEND_BLOCK_ALLOWED_RUNTIMES: &[&str] = &["native_react", "isolated_iframe"];
const FRONTEND_BLOCK_ALLOWED_PRIMITIVES: &[&str] = &[
    "text",
    "image",
    "link",
    "button",
    "rich_text",
    "data_record",
];
const FRONTEND_BLOCK_ALLOWED_UI_CAPABILITIES: &[&str] =
    &["responsive", "configurable", "theming", "data_binding"];
const FRONTEND_BLOCK_HOST_MODULE_SOURCES: &[&str] = &[
    "react",
    "antd",
    "antd-style",
    "dayjs",
    "@ant-design/icons",
    "clsx",
];

pub(crate) fn validate_frontend_block_contributions(
    contributions: &[FrontendBlockContributionManifest],
) -> FrameworkResult<()> {
    if contributions.is_empty() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "capability_plugin must declare block_contributions",
        ));
    }

    let mut registered_module_identities = BTreeMap::new();
    let mut registered_asset_media_types = BTreeMap::new();
    for contribution in contributions {
        validate_non_empty(
            &contribution.contribution_code,
            "block_contributions[].contribution_code",
        )?;
        validate_non_empty(&contribution.title, "block_contributions[].title")?;
        validate_non_empty(&contribution.entry, "block_contributions[].entry")?;
        validate_allowed(
            &contribution.ui_registration,
            "block_contributions[].ui_registration",
            &["code_template", "component"],
        )?;
        if contribution.ui_registration == "component"
            && (contribution.runtime != "native_react" || contribution.code_template.is_none())
        {
            return Err(PluginFrameworkError::invalid_provider_package(
                "block_contributions[].ui_registration component requires native_react and code_template",
            ));
        }
        match (
            contribution.code_template.as_deref(),
            contribution.code_template_version.as_deref(),
            contribution.code_template_language.as_deref(),
        ) {
            (Some(code_template), Some(code_template_version), Some(code_template_language)) => {
                validate_non_empty(code_template, "block_contributions[].code_template")?;
                validate_non_empty(
                    code_template_version,
                    "block_contributions[].code_template_version",
                )?;
                validate_allowed(
                    code_template_language,
                    "block_contributions[].code_template_language",
                    &["jsx", "tsx"],
                )?;
                if code_template.len() > 256 * 1024 {
                    return Err(PluginFrameworkError::invalid_provider_package(
                        "block_contributions[].code_template exceeds 262144 bytes",
                    ));
                }
            }
            (Some(_), None, _) => {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "block_contributions[].code_template_version is required when code_template is set",
                ));
            }
            (None, Some(_), _) | (None, None, Some(_)) => {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "block_contributions[].code_template is required when code_template_version is set",
                ));
            }
            (Some(_), Some(_), None) => {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "block_contributions[].code_template_language is required when code_template is set",
                ));
            }
            (None, None, None) => {}
        }
        validate_allowed(
            &contribution.runtime,
            "block_contributions[].runtime",
            FRONTEND_BLOCK_ALLOWED_RUNTIMES,
        )?;
        validate_isolated_frontend_block_runtime(contribution)?;
        validate_frontend_code_modules_with_identities(
            &contribution.code_modules,
            &mut registered_module_identities,
            &mut registered_asset_media_types,
        )?;
        validate_frontend_block_permissions(&contribution.permissions)?;
        for primitive in &contribution.context_contract.primitives {
            validate_allowed(
                primitive,
                "block_contributions[].context_contract.primitives[]",
                FRONTEND_BLOCK_ALLOWED_PRIMITIVES,
            )?;
        }
        for capability in &contribution.ui_capabilities {
            validate_allowed(
                capability,
                "block_contributions[].ui_capabilities[]",
                FRONTEND_BLOCK_ALLOWED_UI_CAPABILITIES,
            )?;
        }
        if contribution.context_contract.input_schema.is_null() {
            return Err(PluginFrameworkError::invalid_provider_package(
                "block_contributions[].context_contract.input_schema cannot be null",
            ));
        }
    }

    Ok(())
}

fn validate_isolated_frontend_block_runtime(
    contribution: &FrontendBlockContributionManifest,
) -> FrameworkResult<()> {
    if contribution.runtime != "isolated_iframe" {
        return Ok(());
    }
    if contribution.code_modules.len() != 1 {
        return Err(PluginFrameworkError::invalid_provider_package(
            "isolated_iframe block contributions require exactly one entry code module",
        ));
    }
    let entry_module = &contribution.code_modules[0];
    if entry_module.binding != FrontendModuleBindingManifest::Fetched {
        return Err(PluginFrameworkError::invalid_provider_package(
            "isolated_iframe entry code module must use fetched binding",
        ));
    }
    if entry_module.source != contribution.entry {
        return Err(PluginFrameworkError::invalid_provider_package(
            "isolated_iframe entry must match its fetched code module source",
        ));
    }
    let browser_module_count = entry_module
        .assets
        .iter()
        .filter(|asset| asset.role == FrontendModuleAssetRoleManifest::BrowserModule)
        .count();
    if browser_module_count != 1 {
        return Err(PluginFrameworkError::invalid_provider_package(
            "isolated_iframe entry code module requires exactly one browser_module asset",
        ));
    }
    Ok(())
}

pub(crate) fn validate_frontend_code_modules(
    modules: &[FrontendBlockCodeModuleManifest],
) -> FrameworkResult<()> {
    validate_frontend_code_modules_with_identities(
        modules,
        &mut BTreeMap::new(),
        &mut BTreeMap::new(),
    )
}

fn validate_frontend_code_modules_with_identities<'a>(
    modules: &'a [FrontendBlockCodeModuleManifest],
    registered_module_identities: &mut BTreeMap<&'a str, &'a FrontendBlockCodeModuleManifest>,
    registered_asset_media_types: &mut BTreeMap<&'a String, &'a String>,
) -> FrameworkResult<()> {
    let mut local_sources = HashSet::new();
    for code_module in modules {
        validate_non_empty(
            &code_module.source,
            "block_contributions[].code_modules[].source",
        )?;
        validate_non_empty(
            &code_module.version,
            "block_contributions[].code_modules[].version",
        )?;
        if !local_sources.insert(code_module.source.as_str()) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "block_contributions[].code_modules[].source must be unique",
            ));
        }
        // Sibling components may share a package module, but not redefine it.
        // Each contribution retains its own complete asset/host declaration so
        // authorizing one component does not depend on a sibling grant.
        if let Some(previous) =
            registered_module_identities.insert(code_module.source.as_str(), code_module)
        {
            if previous != code_module {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "shared frontend module declarations must be identical",
                ));
            }
        }
        validate_frontend_module_exports(code_module)?;
        validate_frontend_module_binding(code_module)?;
        for asset in &code_module.assets {
            if let Some(existing_media_type) =
                registered_asset_media_types.insert(&asset.sha256, &asset.media_type)
            {
                if existing_media_type != &asset.media_type {
                    return Err(PluginFrameworkError::invalid_provider_package(
                        "one frontend asset digest cannot declare multiple media types",
                    ));
                }
            }
        }
        validate_non_empty(
            &code_module.type_declarations,
            "block_contributions[].code_modules[].type_declarations",
        )?;
    }
    Ok(())
}

fn validate_frontend_module_exports(
    code_module: &FrontendBlockCodeModuleManifest,
) -> FrameworkResult<()> {
    if code_module.exports.is_empty() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "block_contributions[].code_modules[].exports must not be empty",
        ));
    }
    let mut module_exports = HashSet::new();
    for export_name in &code_module.exports {
        validate_javascript_export_name(
            export_name,
            "block_contributions[].code_modules[].exports[]",
        )?;
        if !module_exports.insert(export_name.as_str()) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "block_contributions[].code_modules[].exports[] must be unique",
            ));
        }
    }
    Ok(())
}

fn validate_frontend_module_binding(
    module: &FrontendBlockCodeModuleManifest,
) -> FrameworkResult<()> {
    match module.binding {
        FrontendModuleBindingManifest::Host if !module.assets.is_empty() => {
            return Err(PluginFrameworkError::invalid_provider_package(
                "host frontend modules must not register fetched assets",
            ));
        }
        FrontendModuleBindingManifest::Host => {
            if !FRONTEND_BLOCK_HOST_MODULE_SOURCES.contains(&module.source.as_str()) {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "frontend host module source is not part of the Host ABI",
                ));
            }
            return Ok(());
        }
        FrontendModuleBindingManifest::Fetched => {}
    }

    let mut paths = HashSet::new();
    let mut browser_module_count = 0;
    for asset in &module.assets {
        validate_registered_asset_path(&asset.path)?;
        validate_non_empty(
            &asset.media_type,
            "block_contributions[].code_modules[].assets[].media_type",
        )?;
        validate_frontend_asset_media_type(&asset.media_type)?;
        validate_sha256(&asset.sha256)?;
        if !paths.insert(asset.path.as_str()) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "frontend module asset paths must be unique within one module",
            ));
        }
        match asset.role {
            FrontendModuleAssetRoleManifest::BrowserModule => {
                browser_module_count += 1;
                if asset.media_type != "text/javascript; charset=utf-8" {
                    return Err(PluginFrameworkError::invalid_provider_package(
                        "browser_module frontend assets must use text/javascript; charset=utf-8",
                    ));
                }
            }
            FrontendModuleAssetRoleManifest::ShadowStyle => {
                if asset.media_type != "text/css; charset=utf-8" {
                    return Err(PluginFrameworkError::invalid_provider_package(
                        "shadow_style frontend assets must use text/css; charset=utf-8",
                    ));
                }
            }
            FrontendModuleAssetRoleManifest::Support => {}
        }
    }
    if browser_module_count != 1 {
        return Err(PluginFrameworkError::invalid_provider_package(
            "fetched frontend modules require exactly one browser_module asset",
        ));
    }
    Ok(())
}

fn validate_frontend_asset_media_type(value: &str) -> FrameworkResult<()> {
    let essence = value.split(';').next().unwrap_or_default().trim();
    let Some((type_name, subtype)) = essence.split_once('/') else {
        return Err(PluginFrameworkError::invalid_provider_package(
            "frontend module asset media_type must be a valid MIME type",
        ));
    };
    let valid_token = |token: &str| {
        !token.is_empty()
            && token.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(
                        byte,
                        b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-'
                    )
            })
    };
    if !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
        || !valid_token(type_name)
        || !valid_token(subtype)
    {
        return Err(PluginFrameworkError::invalid_provider_package(
            "frontend module asset media_type must be a valid MIME type",
        ));
    }
    Ok(())
}

fn validate_javascript_export_name(value: &str, field: &str) -> FrameworkResult<()> {
    let mut chars = value.chars();
    let valid_start = chars.next().is_some_and(|character| {
        character == '_' || character == '$' || character.is_ascii_alphabetic()
    });
    if !valid_start
        || !chars.all(|character| {
            character == '_' || character == '$' || character.is_ascii_alphanumeric()
        })
    {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field} must be a JavaScript export name"
        )));
    }
    Ok(())
}

fn validate_registered_asset_path(value: &str) -> FrameworkResult<()> {
    validate_non_empty(value, "block_contributions[].code_modules[].assets[].path")?;
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(PluginFrameworkError::invalid_provider_package(
            "block_contributions[].code_modules[].assets[].path must stay within the plugin package",
        ));
    }
    Ok(())
}

fn validate_sha256(value: &str) -> FrameworkResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(PluginFrameworkError::invalid_provider_package(
            "block_contributions[].code_modules[].assets[].sha256 must be a lowercase SHA-256 digest",
        ));
    }
    Ok(())
}

fn validate_frontend_block_permissions(
    permissions: &FrontendBlockPermissionsManifest,
) -> FrameworkResult<()> {
    validate_allowed(
        &permissions.network,
        "block_contributions[].permissions.network",
        &["none", "outbound_only"],
    )?;
    validate_allowed(
        &permissions.storage,
        "block_contributions[].permissions.storage",
        &["none"],
    )?;
    validate_allowed(
        &permissions.secrets,
        "block_contributions[].permissions.secrets",
        &["none"],
    )?;
    Ok(())
}
