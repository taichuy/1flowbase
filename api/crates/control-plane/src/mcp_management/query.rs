use super::*;

pub(crate) fn validate_identifier(value: &str, field: &'static str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 255 {
        return Err(ControlPlaneError::InvalidInput(field).into());
    }
    Ok(())
}

pub(crate) fn validate_path(value: &str) -> Result<()> {
    if !value.starts_with('/') || value.len() > 255 {
        return Err(ControlPlaneError::InvalidInput("path").into());
    }
    Ok(())
}

pub(crate) fn validate_group_path(value: &str) -> Result<()> {
    if value.len() > 255
        || value == "/"
        || !value.starts_with('/')
        || value.ends_with('/')
        || value.split('/').skip(1).any(|segment| {
            segment.is_empty()
                || !segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
    {
        return Err(ControlPlaneError::InvalidInput("path").into());
    }
    Ok(())
}

pub(crate) fn normalize_group_display_name(path: &str, value: &str) -> Result<String> {
    let display_name = value.trim();
    let display_name = if display_name.is_empty() {
        path.rsplit('/').next().unwrap_or_default()
    } else {
        display_name
    };
    if display_name.is_empty() || display_name.chars().count() > 255 {
        return Err(ControlPlaneError::InvalidInput("display_name").into());
    }
    Ok(display_name.to_owned())
}

pub(crate) fn validate_positive(value: i32, field: &'static str) -> Result<()> {
    if value <= 0 {
        return Err(ControlPlaneError::InvalidInput(field).into());
    }
    Ok(())
}

pub(super) fn validate_list_return_fields(value: &serde_json::Value) -> Result<()> {
    let Some(fields) = value.as_array() else {
        return Err(ControlPlaneError::InvalidInput("list_return_fields").into());
    };
    if fields.is_empty() {
        return Err(ControlPlaneError::InvalidInput("list_return_fields").into());
    }

    let mut seen = BTreeSet::new();
    for field in fields {
        let Some(field) = field.as_str() else {
            return Err(ControlPlaneError::InvalidInput("list_return_fields").into());
        };
        if ![
            "id",
            "type",
            "item_kind",
            "path",
            "name",
            "description_short",
            "children_count",
            "risk_level",
        ]
        .contains(&field)
            || !seen.insert(field)
        {
            return Err(ControlPlaneError::InvalidInput("list_return_fields").into());
        }
    }
    Ok(())
}

pub(super) fn generate_short_id() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_";
    let mut output = String::with_capacity(8);
    for _ in 0..8 {
        let index = (OsRng.next_u32() as usize) % ALPHABET.len();
        output.push(ALPHABET[index] as char);
    }
    output
}

pub(crate) fn normalize_des_id(value: Option<String>) -> String {
    let trimmed = value.unwrap_or_default().trim().to_owned();
    if trimmed.is_empty() {
        generate_short_id()
    } else {
        trimmed
    }
}

fn path_matches(base_path: &str, candidate: &str) -> bool {
    base_path == "/" || candidate == base_path || candidate.starts_with(&format!("{base_path}/"))
}

pub(super) fn parent_group_path(path: &str) -> Option<&str> {
    let path = path.trim_end_matches('/');
    if path.is_empty() || path == "/" {
        return None;
    }
    let separator = path.rfind('/')?;
    if separator == 0 {
        Some("/")
    } else {
        Some(&path[..separator])
    }
}

pub(super) fn list_item_matches_keywords(
    keywords: Option<&[String]>,
    path: &str,
    name: &str,
    description_short: Option<&str>,
) -> bool {
    let searchable = format!(
        "{} {} {}",
        path,
        name,
        description_short.unwrap_or_default()
    )
    .to_lowercase();
    keywords
        .unwrap_or_default()
        .iter()
        .filter(|keyword| !keyword.trim().is_empty())
        .all(|keyword| searchable.contains(&keyword.to_lowercase()))
}

pub(super) fn path_matches_list_query(
    base_path: &str,
    candidate: &str,
    max_depth: i32,
    path_regex_filter: Option<&Regex>,
) -> bool {
    let Some(depth) = list_relative_depth(base_path, candidate) else {
        return false;
    };
    if depth > max_depth {
        return false;
    }
    path_regex_filter
        .map(|path_regex_filter| path_regex_filter.is_match(candidate))
        .unwrap_or(true)
}

fn list_relative_depth(base_path: &str, candidate: &str) -> Option<i32> {
    if !path_matches(base_path, candidate) {
        return None;
    }
    if candidate == base_path {
        return Some(0);
    }
    let relative_path = if base_path == "/" {
        candidate.trim_start_matches('/')
    } else {
        candidate.strip_prefix(base_path)?.trim_start_matches('/')
    };
    Some(
        relative_path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .count() as i32,
    )
}

pub(super) fn compile_list_path_regex(
    pattern: Option<&str>,
    regex_enabled: bool,
    regex_max_length: i32,
) -> Result<Option<Regex>> {
    let Some(pattern) = pattern else {
        return Ok(None);
    };
    if !regex_enabled {
        return Err(ControlPlaneError::InvalidInput("path_regex").into());
    }
    let regex_max_length = usize::try_from(regex_max_length)
        .map_err(|_| ControlPlaneError::InvalidInput("path_regex"))?;
    if pattern.chars().count() > regex_max_length {
        return Err(ControlPlaneError::InvalidInput("path_regex").into());
    }
    Regex::new(pattern)
        .map(Some)
        .map_err(|_| ControlPlaneError::InvalidInput("path_regex").into())
}

pub(super) fn bindable_interface(
    entry: domain::McpInterfaceCatalogEntry,
) -> Result<domain::McpInterfaceCatalogEntry> {
    if !entry.bindable {
        return Err(ControlPlaneError::InvalidInput("interface_id").into());
    }
    Ok(entry)
}

/// Apply the existing MCP discovery policy to a supplied (possibly Host-projected) catalog.
#[allow(clippy::too_many_arguments)]
pub fn list_catalog_items(
    catalog: &domain::McpCatalogSnapshot,
    instance_id: &str,
    path: Option<&str>,
    path_regex: Option<&str>,
    keywords: Option<&[String]>,
    depth: Option<i32>,
    limit: Option<usize>,
) -> Result<Vec<domain::McpListItemSummary>> {
    let instance = catalog
        .instances
        .iter()
        .find(|i| i.instance_id == instance_id && i.status == domain::McpInstanceStatus::Enabled)
        .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
    let discovery_policy = catalog
        .discovery_policies
        .iter()
        .find(|p| p.instance_record_id == instance.id)
        .ok_or(ControlPlaneError::NotFound("mcp_instance_discovery_policy"))?;
    let path_regex_filter = compile_list_path_regex(
        path_regex,
        discovery_policy.list_regex_enabled,
        discovery_policy.list_regex_max_length,
    )?;
    let groups = catalog
        .groups
        .iter()
        .filter(|g| g.instance_record_id == instance.id)
        .cloned()
        .collect::<Vec<_>>();
    let bindings = catalog
        .bindings
        .iter()
        .filter(|b| b.instance_record_id == instance.id)
        .cloned()
        .collect::<Vec<_>>();
    let tools = &catalog.tools;
    let enabled_tool_ids = tools
        .iter()
        .filter(|tool| tool.status == domain::McpToolStatus::Enabled)
        .map(|tool| tool.id)
        .collect::<HashSet<_>>();
    let mut children_counts = HashMap::<String, i64>::new();
    for group in groups.iter().filter(|group| group.enabled) {
        if let Some(parent_path) = parent_group_path(&group.path) {
            *children_counts.entry(parent_path.to_owned()).or_default() += 1;
        }
    }
    for binding in bindings
        .iter()
        .filter(|binding| binding.visible && enabled_tool_ids.contains(&binding.tool_record_id))
    {
        *children_counts
            .entry(binding.group_path.clone())
            .or_default() += 1;
    }
    let base_path = path.unwrap_or(instance.default_entry_path.as_str());
    let max_depth = depth
        .unwrap_or(discovery_policy.list_max_depth)
        .clamp(0, discovery_policy.list_max_depth);
    let mut items = Vec::new();

    for group in groups.into_iter().filter(|group| {
        group.enabled
            && path_matches_list_query(
                base_path,
                &group.path,
                max_depth,
                path_regex_filter.as_ref(),
            )
    }) {
        if list_item_matches_keywords(
            keywords,
            &group.path,
            &group.display_name,
            group.description_short.as_deref(),
        ) {
            let children_count = children_counts
                .get(&group.path)
                .copied()
                .unwrap_or_default();
            items.push(domain::McpListItemSummary {
                id: group.id.to_string(),
                item_kind: domain::McpListItemKind::Group,
                path: group.path,
                name: group.display_name,
                description_short: group.description_short,
                children_count,
                risk_level: None,
            });
        }
    }

    for binding in bindings.into_iter().filter(|binding| {
        binding.visible
            && path_matches_list_query(
                base_path,
                &binding.group_path,
                max_depth,
                path_regex_filter.as_ref(),
            )
    }) {
        if let Some(tool) = tools
            .iter()
            .find(|tool| tool.id == binding.tool_record_id)
            .filter(|tool| tool.status == domain::McpToolStatus::Enabled)
        {
            let display_name = binding
                .display_alias
                .as_deref()
                .unwrap_or(tool.name.as_str());
            if list_item_matches_keywords(
                keywords,
                &binding.group_path,
                display_name,
                Some(&tool.short_description),
            ) {
                items.push(domain::McpListItemSummary {
                    id: tool.tool_id.clone(),
                    item_kind: domain::McpListItemKind::Tool,
                    path: binding.group_path,
                    name: binding
                        .display_alias
                        .clone()
                        .unwrap_or_else(|| tool.name.clone()),
                    description_short: Some(tool.short_description.clone()),
                    children_count: 0,
                    risk_level: Some(tool.risk_level),
                });
            }
        }
    }

    let limit = limit.unwrap_or(discovery_policy.list_default_limit as usize);
    items.truncate(limit);
    Ok(items)
}
