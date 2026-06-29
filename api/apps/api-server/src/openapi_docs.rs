use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Map, Value};
use utoipa::{OpenApi, ToSchema};

const HTTP_METHODS: &[&str] = &[
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];
const DEFAULT_DOCS_SERVER_URL: &str = "/";
const DEFAULT_SESSION_COOKIE_NAME: &str = "flowbase_console_session";
const SESSION_COOKIE_SECURITY_SCHEME: &str = "sessionCookie";
const CSRF_HEADER_SECURITY_SCHEME: &str = "csrfHeader";
const CSRF_HEADER_NAME: &str = "x-csrf-token";
const PAT_BEARER_SECURITY_SCHEME: &str = "patBearer";
pub const DOCS_OPERATIONS_PAGE_SIZE: usize = 20;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DocsCatalogOperation {
    pub id: String,
    pub method: String,
    pub path: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub group: String,
    pub deprecated: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DocsCatalogCategory {
    pub id: String,
    pub label: String,
    pub operation_count: usize,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DocsCatalogCategoryOperations {
    pub id: String,
    pub label: String,
    pub operations: Vec<DocsCatalogOperation>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DocsCatalogCategoryOperationsPage {
    pub id: String,
    pub label: String,
    pub operations: Vec<DocsCatalogOperation>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DocsCatalog {
    pub title: String,
    pub version: String,
    pub categories: Vec<DocsCatalogCategory>,
}

#[derive(Debug, Clone)]
pub struct ApiDocsRegistry {
    catalog: DocsCatalog,
    category_operations: HashMap<String, DocsCatalogCategoryOperations>,
    category_specs: HashMap<String, Value>,
    operation_specs: HashMap<String, Value>,
}

impl ApiDocsRegistry {
    pub fn catalog(&self) -> &DocsCatalog {
        &self.catalog
    }

    pub fn operation_spec(&self, operation_id: &str) -> Option<&Value> {
        self.operation_specs.get(operation_id)
    }

    pub fn category_operations(&self, category_id: &str) -> Option<&DocsCatalogCategoryOperations> {
        self.category_operations.get(category_id)
    }

    pub fn category_spec(&self, category_id: &str) -> Option<&Value> {
        self.category_specs.get(category_id)
    }
}

pub fn build_default_api_docs_registry() -> Result<ApiDocsRegistry> {
    build_default_api_docs_registry_with_cookie_name(DEFAULT_SESSION_COOKIE_NAME)
}

pub fn build_default_api_docs_registry_with_cookie_name(
    cookie_name: &str,
) -> Result<ApiDocsRegistry> {
    build_api_docs_registry_with_cookie_name(
        serde_json::to_value(crate::openapi::ApiDoc::openapi())?,
        cookie_name,
    )
}

pub fn build_api_docs_registry(canonical: Value) -> Result<ApiDocsRegistry> {
    build_api_docs_registry_with_cookie_name(canonical, DEFAULT_SESSION_COOKIE_NAME)
}

pub fn paginate_category_operations(
    category_operations: &DocsCatalogCategoryOperations,
    offset: usize,
    limit: usize,
) -> DocsCatalogCategoryOperationsPage {
    let total = category_operations.operations.len();
    let effective_limit = limit.max(1);
    let start = offset.min(total);
    let end = start.saturating_add(effective_limit).min(total);
    let next_offset = if end < total { Some(end) } else { None };

    DocsCatalogCategoryOperationsPage {
        id: category_operations.id.clone(),
        label: category_operations.label.clone(),
        operations: category_operations.operations[start..end].to_vec(),
        total,
        offset: start,
        limit: effective_limit,
        has_more: next_offset.is_some(),
        next_offset,
    }
}

fn normalize_docs_operation_search_text(input: &str) -> String {
    input
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| !matches!(character, ' ' | '\t' | '\n' | '\r' | '-' | '/' | ':' | '_'))
        .collect()
}

fn operation_search_text(operation: &DocsCatalogOperation) -> String {
    normalize_docs_operation_search_text(&format!(
        "{} {} {} {} {} {}",
        operation.method,
        operation.path,
        operation.summary.as_deref().unwrap_or_default(),
        operation.description.as_deref().unwrap_or_default(),
        operation.group,
        operation.id
    ))
}

pub fn filter_category_operations(
    category_operations: &DocsCatalogCategoryOperations,
    query: Option<&str>,
) -> DocsCatalogCategoryOperations {
    let Some(normalized_query) = query.map(normalize_docs_operation_search_text) else {
        return category_operations.clone();
    };

    if normalized_query.is_empty() {
        return category_operations.clone();
    }

    DocsCatalogCategoryOperations {
        id: category_operations.id.clone(),
        label: category_operations.label.clone(),
        operations: category_operations
            .operations
            .iter()
            .filter(|operation| operation_search_text(operation).contains(&normalized_query))
            .cloned()
            .collect(),
    }
}

fn build_api_docs_registry_with_cookie_name(
    canonical: Value,
    cookie_name: &str,
) -> Result<ApiDocsRegistry> {
    let canonical = enrich_canonical_openapi(canonical, cookie_name)?;
    let canonical_map = canonical
        .as_object()
        .context("canonical OpenAPI document must be a JSON object")?;
    let title = canonical_map
        .get("info")
        .and_then(Value::as_object)
        .and_then(|info| info.get("title"))
        .and_then(Value::as_str)
        .context("canonical OpenAPI document must contain info.title")?
        .to_string();
    let version = canonical_map
        .get("info")
        .and_then(Value::as_object)
        .and_then(|info| info.get("version"))
        .and_then(Value::as_str)
        .context("canonical OpenAPI document must contain info.version")?
        .to_string();
    canonical_map
        .get("openapi")
        .and_then(Value::as_str)
        .context("canonical OpenAPI document must contain openapi")?;

    let paths = canonical_map
        .get("paths")
        .and_then(Value::as_object)
        .context("canonical OpenAPI document must contain paths")?;

    let mut category_operations = HashMap::<String, DocsCatalogCategoryOperations>::new();
    let mut category_members = HashMap::<String, Vec<(String, String)>>::new();
    let mut category_singleton_flags = HashMap::<String, bool>::new();
    let mut category_specs = HashMap::new();
    let mut operation_specs = HashMap::new();
    let mut seen_ids = HashSet::new();

    for (path, path_item) in paths {
        let path_item_map = path_item
            .as_object()
            .with_context(|| format!("path item `{path}` must be an object"))?;

        for method in HTTP_METHODS {
            let Some(operation) = path_item_map.get(*method) else {
                continue;
            };

            if should_hide_generic_runtime_model_crud(path) {
                continue;
            }

            let operation_map = operation
                .as_object()
                .with_context(|| format!("operation `{method} {path}` must be an object"))?;
            let operation_id = operation_map
                .get("operationId")
                .and_then(Value::as_str)
                .with_context(|| format!("operation `{method} {path}` must define operationId"))?;

            if !seen_ids.insert(operation_id.to_string()) {
                bail!("duplicate operationId `{operation_id}`");
            }

            let tags = extract_tags(operation);
            let (category_id, category_label, is_singleton) = derive_category(path, operation_id);
            let catalog_operation = DocsCatalogOperation {
                id: operation_id.to_string(),
                method: method.to_ascii_uppercase(),
                path: path.to_string(),
                summary: operation_map
                    .get("summary")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                description: operation_map
                    .get("description")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                tags: tags.clone(),
                group: category_label.clone(),
                deprecated: operation_map
                    .get("deprecated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            };

            category_singleton_flags.insert(category_id.clone(), is_singleton);
            category_members
                .entry(category_id.clone())
                .or_default()
                .push((path.to_string(), (*method).to_string()));
            category_operations
                .entry(category_id.clone())
                .or_insert_with(|| DocsCatalogCategoryOperations {
                    id: category_id.clone(),
                    label: category_label.clone(),
                    operations: Vec::new(),
                })
                .operations
                .push(catalog_operation);

            operation_specs.insert(
                operation_id.to_string(),
                close_operation_spec(&canonical, path, method)?,
            );
        }
    }

    for operations in category_operations.values_mut() {
        operations.operations.sort_by(compare_operations);
    }

    for (category_id, members) in &category_members {
        category_specs.insert(category_id.clone(), close_scoped_spec(&canonical, members)?);
    }

    let mut categories = category_operations
        .values()
        .map(|operations| DocsCatalogCategory {
            id: operations.id.clone(),
            label: operations.label.clone(),
            operation_count: operations.operations.len(),
        })
        .collect::<Vec<_>>();
    categories.sort_by(|left, right| compare_categories(left, right, &category_singleton_flags));

    Ok(ApiDocsRegistry {
        catalog: DocsCatalog {
            title,
            version,
            categories,
        },
        category_operations,
        category_specs,
        operation_specs,
    })
}

fn enrich_canonical_openapi(mut canonical: Value, cookie_name: &str) -> Result<Value> {
    let canonical_map = canonical
        .as_object_mut()
        .context("canonical OpenAPI document must be a JSON object")?;

    ensure_docs_server(canonical_map);
    ensure_security_schemes(canonical_map, cookie_name)?;
    ensure_shared_crud_protocol_components(canonical_map)?;
    annotate_console_operation_security(canonical_map)?;

    Ok(canonical)
}

fn extract_tags(operation: &Value) -> Vec<String> {
    operation
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn derive_category(path: &str, operation_id: &str) -> (String, String, bool) {
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    match segments.as_slice() {
        ["api", category, ..] if !category.starts_with('{') => {
            let category = (*category).to_string();
            (category.clone(), category, false)
        }
        _ => (format!("single:{operation_id}"), path.to_string(), true),
    }
}

fn should_hide_generic_runtime_model_crud(path: &str) -> bool {
    matches!(
        path,
        "/api/runtime/models/{model_code}/list"
            | "/api/runtime/models/{model_code}/get/{id}"
            | "/api/runtime/models/{model_code}/create"
            | "/api/runtime/models/{model_code}/update/{id}"
            | "/api/runtime/models/{model_code}/delete/{id}"
    )
}

fn compare_operations(
    left: &DocsCatalogOperation,
    right: &DocsCatalogOperation,
) -> std::cmp::Ordering {
    left.path
        .cmp(&right.path)
        .then_with(|| left.method.cmp(&right.method))
        .then_with(|| left.id.cmp(&right.id))
}

fn compare_categories(
    left: &DocsCatalogCategory,
    right: &DocsCatalogCategory,
    singleton_flags: &HashMap<String, bool>,
) -> std::cmp::Ordering {
    let left_is_singleton = singleton_flags.get(&left.id).copied().unwrap_or(false);
    let right_is_singleton = singleton_flags.get(&right.id).copied().unwrap_or(false);

    left_is_singleton
        .cmp(&right_is_singleton)
        .then_with(|| left.label.cmp(&right.label))
        .then_with(|| left.id.cmp(&right.id))
}

fn ensure_docs_server(canonical_map: &mut Map<String, Value>) {
    let has_servers = canonical_map
        .get("servers")
        .and_then(Value::as_array)
        .map(|servers| !servers.is_empty())
        .unwrap_or(false);

    if !has_servers {
        canonical_map.insert(
            "servers".to_string(),
            json!([
                {
                    "url": DEFAULT_DOCS_SERVER_URL,
                    "description": "Current API server"
                }
            ]),
        );
    }
}

fn ensure_security_schemes(
    canonical_map: &mut Map<String, Value>,
    cookie_name: &str,
) -> Result<()> {
    let components = canonical_map
        .entry("components".to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .context("components must be an object")?;
    let security_schemes = components
        .entry("securitySchemes".to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .context("components.securitySchemes must be an object")?;

    security_schemes
        .entry(SESSION_COOKIE_SECURITY_SCHEME.to_string())
        .or_insert_with(|| {
            json!({
                "type": "apiKey",
                "in": "cookie",
                "name": cookie_name,
                "description": "Console session cookie for authenticated requests."
            })
        });
    security_schemes
        .entry(CSRF_HEADER_SECURITY_SCHEME.to_string())
        .or_insert_with(|| {
            json!({
                "type": "apiKey",
                "in": "header",
                "name": CSRF_HEADER_NAME,
                "description": "CSRF token header required by mutating console operations."
            })
        });
    security_schemes
        .entry(PAT_BEARER_SECURITY_SCHEME.to_string())
        .or_insert_with(|| {
            json!({
                "type": "http",
                "scheme": "bearer",
                "bearerFormat": "pat_ user API key",
                "description": "Use Authorization: Bearer pat_... for user API key requests. PAT requests use the bound user's role permissions and do not require CSRF."
            })
        });

    Ok(())
}

fn ensure_shared_crud_protocol_components(canonical_map: &mut Map<String, Value>) -> Result<()> {
    let components = canonical_map
        .entry("components".to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .context("components must be an object")?;
    let parameters = components
        .entry("parameters".to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .context("components.parameters must be an object")?;
    parameters
        .entry("CrudFilter".to_string())
        .or_insert_with(|| {
            json!({
                "name": "filter",
                "in": "query",
                "required": false,
                "schema": { "type": "string" },
                "description": "JSON filter expression. Supports field operators such as $eq, $ne, $gt, $gte, $lt, $lte, $includes, $notIncludes and $in. URL-encode the JSON string in query parameters."
            })
        });
    parameters.entry("CrudSort".to_string()).or_insert_with(|| {
        json!({
            "name": "sort",
            "in": "query",
            "required": false,
            "schema": { "type": "string" },
            "description": "Sort expression using field:asc or field:desc."
        })
    });
    parameters.entry("CrudPage".to_string()).or_insert_with(|| {
        json!({
            "name": "page",
            "in": "query",
            "required": false,
            "schema": { "type": "integer", "minimum": 1, "default": 1 },
            "description": "Page number."
        })
    });
    parameters
        .entry("CrudPageSize".to_string())
        .or_insert_with(|| {
            json!({
                "name": "page_size",
                "in": "query",
                "required": false,
                "schema": { "type": "integer", "minimum": 1, "default": 20 },
                "description": "Page size."
            })
        });

    let schemas = components
        .entry("schemas".to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .context("components.schemas must be an object")?;
    schemas
        .entry("CrudBatchSelection".to_string())
        .or_insert_with(|| {
            json!({
                "type": "object",
                "properties": {
                    "filterByTk": {
                        "oneOf": [
                            { "type": "string" },
                            { "type": "array", "items": { "type": "string" } }
                        ],
                        "description": "Selected primary key value or values."
                    },
                    "filter": {
                        "type": "object",
                        "additionalProperties": true,
                        "description": "JSON filter expression for conditional batch operations."
                    }
                }
            })
        });

    Ok(())
}

fn annotate_console_operation_security(canonical_map: &mut Map<String, Value>) -> Result<()> {
    let paths = canonical_map
        .get_mut("paths")
        .and_then(Value::as_object_mut)
        .context("canonical OpenAPI document must contain paths")?;

    for (path, path_item) in paths {
        if !path.starts_with("/api/console/") {
            continue;
        }

        let path_item_map = path_item
            .as_object_mut()
            .with_context(|| format!("path item `{path}` must be an object"))?;

        for method in HTTP_METHODS {
            let Some(operation) = path_item_map.get_mut(*method) else {
                continue;
            };
            let operation_map = operation
                .as_object_mut()
                .with_context(|| format!("operation `{method} {path}` must be an object"))?;

            operation_map
                .entry("security".to_string())
                .or_insert_with(|| derive_console_security_requirement(method));
        }
    }

    Ok(())
}

fn derive_console_security_requirement(method: &str) -> Value {
    if requires_csrf(method) {
        json!([
            {
                SESSION_COOKIE_SECURITY_SCHEME: [],
                CSRF_HEADER_SECURITY_SCHEME: []
            },
            {
                PAT_BEARER_SECURITY_SCHEME: []
            }
        ])
    } else {
        json!([
            {
                SESSION_COOKIE_SECURITY_SCHEME: []
            },
            {
                PAT_BEARER_SECURITY_SCHEME: []
            }
        ])
    }
}

fn requires_csrf(method: &str) -> bool {
    !matches!(method, "get" | "head" | "options")
}

pub fn collect_refs(value: &Value, refs: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(target) = map.get("$ref").and_then(Value::as_str) {
                refs.insert(target.to_string());
            }
            for nested in map.values() {
                collect_refs(nested, refs);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_refs(item, refs);
            }
        }
        _ => {}
    }
}

fn close_operation_spec(canonical: &Value, path: &str, method: &str) -> Result<Value> {
    close_scoped_spec(canonical, &[(path.to_string(), method.to_string())])
}

fn close_scoped_spec(canonical: &Value, scoped_operations: &[(String, String)]) -> Result<Value> {
    let canonical_map = canonical
        .as_object()
        .context("canonical OpenAPI document must be a JSON object")?;
    let canonical_paths = canonical_map
        .get("paths")
        .and_then(Value::as_object)
        .context("canonical OpenAPI document must contain paths")?;

    let mut scoped_paths = Map::new();
    let mut operation_tags = BTreeSet::new();
    let scoped_security = collect_scoped_security_requirements(canonical_map, scoped_operations)?;
    let scoped_security_scheme_names = collect_security_scheme_names(&scoped_security);

    for (path, method) in scoped_operations {
        let path_item_map = canonical_paths
            .get(path)
            .and_then(Value::as_object)
            .with_context(|| format!("path `{path}` not found in canonical document"))?;
        let scoped_path_item = scoped_paths
            .entry(path.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        let scoped_path_item_map = scoped_path_item
            .as_object_mut()
            .context("scoped path item must be an object")?;

        for (key, value) in path_item_map {
            if key == method || !HTTP_METHODS.contains(&key.as_str()) {
                scoped_path_item_map
                    .entry(key.clone())
                    .or_insert_with(|| value.clone());
            }
        }

        let operation = path_item_map.get(method).with_context(|| {
            format!("operation `{method} {path}` not found in canonical document")
        })?;
        operation_tags.extend(extract_tags(operation));
    }

    let mut refs = BTreeSet::new();
    collect_refs(&Value::Object(scoped_paths.clone()), &mut refs);

    let mut components = Value::Object(Map::new());
    let mut pending_refs = refs.iter().cloned().collect::<Vec<_>>();
    let mut visited_refs = HashSet::new();

    while let Some(reference) = pending_refs.pop() {
        if !visited_refs.insert(reference.clone()) {
            continue;
        }

        let pointer = reference
            .strip_prefix('#')
            .ok_or_else(|| anyhow!("unsupported external $ref `{reference}`"))?;
        let referenced_value = canonical
            .pointer(pointer)
            .with_context(|| format!("missing referenced node `{reference}`"))?
            .clone();

        if let Some(component_pointer) = pointer.strip_prefix("/components") {
            insert_pointer(&mut components, component_pointer, referenced_value.clone())?;
        } else {
            bail!("unsupported non-components $ref `{reference}`");
        }

        let mut nested_refs = BTreeSet::new();
        collect_refs(&referenced_value, &mut nested_refs);
        pending_refs.extend(nested_refs);
    }

    for scheme_name in scoped_security_scheme_names {
        let security_scheme = canonical
            .pointer(&format!(
                "/components/securitySchemes/{}",
                scheme_name.replace('~', "~0").replace('/', "~1")
            ))
            .with_context(|| format!("missing security scheme `{scheme_name}`"))?
            .clone();
        insert_pointer(
            &mut components,
            &format!("/securitySchemes/{scheme_name}"),
            security_scheme,
        )?;
    }

    let filtered_tags = canonical_map
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter(|tag| {
                    tag.get("name")
                        .and_then(Value::as_str)
                        .map(|name| operation_tags.contains(name))
                        .unwrap_or(false)
                })
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut spec = Map::new();
    spec.insert(
        "openapi".to_string(),
        canonical_map
            .get("openapi")
            .cloned()
            .context("canonical OpenAPI document must contain openapi")?,
    );
    spec.insert(
        "info".to_string(),
        canonical_map
            .get("info")
            .cloned()
            .context("canonical OpenAPI document must contain info")?,
    );
    if let Some(servers) = canonical_map.get("servers") {
        spec.insert("servers".to_string(), servers.clone());
    }
    if !scoped_security.is_empty() {
        spec.insert("security".to_string(), Value::Array(scoped_security));
    }
    spec.insert("paths".to_string(), Value::Object(scoped_paths));
    spec.insert("components".to_string(), components);
    if !filtered_tags.is_empty() {
        spec.insert("tags".to_string(), Value::Array(filtered_tags));
    }

    Ok(Value::Object(spec))
}

fn collect_scoped_security_requirements(
    canonical_map: &Map<String, Value>,
    scoped_operations: &[(String, String)],
) -> Result<Vec<Value>> {
    let canonical_paths = canonical_map
        .get("paths")
        .and_then(Value::as_object)
        .context("canonical OpenAPI document must contain paths")?;
    let default_security = canonical_map.get("security").and_then(Value::as_array);
    let mut collected = Vec::new();
    let mut seen = BTreeSet::new();

    for (path, method) in scoped_operations {
        let path_item_map = canonical_paths
            .get(path)
            .and_then(Value::as_object)
            .with_context(|| format!("path `{path}` not found in canonical document"))?;
        let operation = path_item_map.get(method).with_context(|| {
            format!("operation `{method} {path}` not found in canonical document")
        })?;
        let requirements = operation
            .get("security")
            .and_then(Value::as_array)
            .or(default_security);

        let Some(requirements) = requirements else {
            continue;
        };

        for requirement in requirements {
            let signature = serde_json::to_string(requirement)?;
            if seen.insert(signature) {
                collected.push(requirement.clone());
            }
        }
    }

    Ok(collected)
}

fn collect_security_scheme_names(requirements: &[Value]) -> BTreeSet<String> {
    let mut scheme_names = BTreeSet::new();

    for requirement in requirements {
        let Some(requirement_map) = requirement.as_object() else {
            continue;
        };

        scheme_names.extend(requirement_map.keys().cloned());
    }

    scheme_names
}

fn insert_pointer(target: &mut Value, pointer: &str, value: Value) -> Result<()> {
    if pointer.is_empty() {
        *target = value;
        return Ok(());
    }

    let mut current = target;
    let mut tokens = pointer
        .trim_start_matches('/')
        .split('/')
        .map(unescape_json_pointer_token)
        .peekable();

    while let Some(token) = tokens.next() {
        let is_last = tokens.peek().is_none();
        let map = current
            .as_object_mut()
            .context("JSON pointer target must be an object")?;

        if is_last {
            map.insert(token, value);
            return Ok(());
        }

        current = map
            .entry(token)
            .or_insert_with(|| Value::Object(Map::new()));
    }

    Ok(())
}

fn unescape_json_pointer_token(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}
