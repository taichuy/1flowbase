use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use control_plane::frontstage::{etag_matches, FrontstagePageService};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_settings_feature_permission::{
        authorize_compiled_console_access, compiled_console_route_access,
    },
    response::ApiSuccess,
    routes::console_interface::ConsoleLocaleHints,
};

use super::{
    block_tree::{
        to_code_response, to_node_response, FrontstageBlockNodeCodeResponse,
        FrontstageBlockNodeResponse,
    },
    interface_pages::{self, FrontstagePagesDependencies},
    invoke_pages, to_page_detail_response, to_tab_response, to_tree_node_response,
    FrontstageNavigationPlacementResponse, FrontstagePageDetailResponse, FrontstagePageTabResponse,
    FrontstagePageTreeNodeKind, FrontstagePageTreeNodeResponse,
};

const DEFAULT_BOOTSTRAP_BLOCK_LIMIT: u32 = 100;

#[derive(Debug, Clone, Default, Deserialize, IntoParams, Serialize)]
pub struct FrontstageBootstrapQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_limit: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageBootstrapResponse {
    pub pages: Vec<FrontstagePageTreeNodeResponse>,
    pub resolved_page_id: Option<String>,
    pub resolved_tab_id: Option<String>,
    pub tabs: Vec<FrontstagePageTabResponse>,
    pub page_detail: Option<FrontstagePageDetailResponse>,
    pub block_roots: Option<Vec<FrontstageBlockNodeResponse>>,
    pub block_codes: Option<Vec<FrontstageBlockNodeCodeResponse>>,
}

pub(crate) enum FrontstageBootstrapRead {
    Modified {
        etag: String,
        payload: Box<FrontstageBootstrapResponse>,
    },
    NotModified(String),
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/bootstrap",
    params(FrontstageBootstrapQuery),
    summary = "Bootstrap Frontstage workspace landing and visible block source codes",
    description = "Resolves the visible Frontstage page tree, target landing page, localized tabs, active page document, and—when the caller's role permits block/code read operations—the root block tree and live database source code in a single HTTP round-trip.",
    responses(
        (status = 200, body = FrontstageBootstrapResponse),
        (status = 304, description = "Workspace landing and block revisions unchanged"),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_frontstage_bootstrap(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<FrontstageBootstrapQuery>,
) -> Result<Response, ApiError> {
    let locale = ConsoleLocaleHints::from_headers(&headers);
    let if_none_match = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let output = invoke_pages(
        state,
        headers,
        "http.console.frontstage.bootstrap.get.v1",
        interface_pages::FrontstagePagesInput::Bootstrap(query, locale, if_none_match),
        false,
    )
    .await?;

    let (etag, mut response) = match output {
        interface_pages::FrontstagePagesOutput::Bootstrap { etag, payload } => {
            (etag, Json(ApiSuccess::new(*payload)).into_response())
        }
        interface_pages::FrontstagePagesOutput::BootstrapNotModified(etag) => {
            (etag, StatusCode::NOT_MODIFIED.into_response())
        }
        _ => unreachable!(),
    };

    let etag_header = HeaderValue::from_str(&format!("\"{etag}\""))
        .map_err(|error| ApiError(anyhow::Error::from(error)))?;
    response.headers_mut().insert(header::ETAG, etag_header);
    // Revalidate against current ACL and live AI-edited block source on every read.
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    response.headers_mut().insert(
        header::VARY,
        HeaderValue::from_static("Accept-Encoding, Cookie, Accept-Language"),
    );
    Ok(response)
}

pub(super) async fn execute_frontstage_bootstrap(
    deps: &FrontstagePagesDependencies,
    service: &FrontstagePageService<storage_durable_postgres::MainDurableStore>,
    actor: &domain::ActorContext,
    query: FrontstageBootstrapQuery,
    locale: ConsoleLocaleHints,
    if_none_match: Option<&str>,
) -> Result<FrontstageBootstrapRead, ApiError> {
    let workspace_id = actor.current_workspace_id;
    let (tree_nodes, snapshot) = service
        .load_visible_page_tree_with_snapshot(actor.user_id, workspace_id)
        .await?;
    let pages: Vec<FrontstagePageTreeNodeResponse> =
        tree_nodes.into_iter().map(to_tree_node_response).collect();

    let resolved_page_id = resolve_target_page_id(&pages, &query);
    let Some(page_id_str) = resolved_page_id.clone() else {
        return finish_bootstrap(
            actor,
            false,
            false,
            0,
            if_none_match,
            FrontstageBootstrapResponse {
                pages,
                resolved_page_id: None,
                resolved_tab_id: None,
                tabs: Vec::new(),
                page_detail: None,
                block_roots: None,
                block_codes: None,
            },
        );
    };

    let Ok(page_uuid) = Uuid::parse_str(&page_id_str) else {
        return finish_bootstrap(
            actor,
            false,
            false,
            0,
            if_none_match,
            FrontstageBootstrapResponse {
                pages,
                resolved_page_id: None,
                resolved_tab_id: None,
                tabs: Vec::new(),
                page_detail: None,
                block_roots: None,
                block_codes: None,
            },
        );
    };

    let (raw_tabs, permissions, user_record) = tokio::try_join!(
        async {
            service
                .list_visible_tabs_in_snapshot(&snapshot, workspace_id, page_uuid)
                .await
                .map_err(ApiError::from)
        },
        evaluate_block_and_code_permissions(deps, actor),
        async {
            deps.store
                .find_user_by_id(actor.user_id)
                .await
                .map_err(ApiError::from)
        },
    )?;
    let (can_view_blocks, can_view_codes) = permissions;
    let mut raw_tabs = raw_tabs;
    let preferred_locale = user_record
        .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
        .preferred_locale;
    let resolved_locale = locale.resolve(preferred_locale);

    // Localize default tab title at most once and reuse across `tabs` and `page_detail`.
    let mut localized_default_titles: HashMap<Uuid, String> = HashMap::new();
    for tab in &mut raw_tabs {
        if tab.is_default {
            let stored = tab.title.as_deref().unwrap_or_default();
            let localized = crate::app_state::project_canonical_display_with(
                &deps.store,
                deps.bootstrap_workspace_id,
                &resolved_locale,
                "Default",
                stored,
            )
            .await?;
            localized_default_titles.insert(tab.id, localized.clone());
            tab.title = Some(localized);
        }
    }
    let tabs: Vec<FrontstagePageTabResponse> =
        raw_tabs.iter().cloned().map(to_tab_response).collect();

    let matched_tab = resolve_target_tab(&tabs, query.tab_reference.as_deref());
    let detail_tab_ref = query
        .tab_reference
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| matched_tab.as_ref().map(|tab| tab.id.clone()));
    let optimistic_tab_uuid = matched_tab
        .as_ref()
        .and_then(|tab| Uuid::parse_str(&tab.id).ok());

    let limit = query.limit.unwrap_or(DEFAULT_BOOTSTRAP_BLOCK_LIMIT);
    let (raw_detail, mut raw_roots) = service
        .load_tab_detail_and_block_roots_in_snapshot(
            &snapshot,
            workspace_id,
            page_uuid,
            detail_tab_ref.as_deref(),
            optimistic_tab_uuid.filter(|_| can_view_blocks),
            limit,
        )
        .await?;

    let mut page_detail = None;
    if let Some(mut detail) = raw_detail {
        if detail.tab.is_default {
            if let Some(cached_title) = localized_default_titles.get(&detail.tab.id) {
                detail.tab.title = Some(cached_title.clone());
            } else {
                let stored = detail.tab.title.as_deref().unwrap_or_default();
                detail.tab.title = Some(
                    crate::app_state::project_canonical_display_with(
                        &deps.store,
                        deps.bootstrap_workspace_id,
                        &resolved_locale,
                        "Default",
                        stored,
                    )
                    .await?,
                );
            }
        }
        page_detail = Some(to_page_detail_response(detail));
    }

    let resolved_tab_id = matched_tab
        .as_ref()
        .map(|tab| tab.id.clone())
        .or_else(|| page_detail.as_ref().map(|detail| detail.tab.id.clone()));

    let Some(tab_uuid) = resolved_tab_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok())
    else {
        return finish_bootstrap(
            actor,
            can_view_blocks,
            can_view_codes,
            0,
            if_none_match,
            FrontstageBootstrapResponse {
                pages,
                resolved_page_id: Some(page_id_str),
                resolved_tab_id,
                tabs,
                page_detail,
                block_roots: None,
                block_codes: None,
            },
        );
    };

    if !can_view_blocks {
        return finish_bootstrap(
            actor,
            can_view_blocks,
            can_view_codes,
            0,
            if_none_match,
            FrontstageBootstrapResponse {
                pages,
                resolved_page_id: Some(page_id_str),
                resolved_tab_id,
                tabs,
                page_detail,
                block_roots: None,
                block_codes: None,
            },
        );
    }

    // Fallback only if `matched_tab` was unknown prior to `detail` resolution.
    if raw_roots.is_none() {
        let (_, late_roots) = service
            .load_tab_detail_and_block_roots_in_snapshot(
                &snapshot,
                workspace_id,
                page_uuid,
                None,
                Some(tab_uuid),
                limit,
            )
            .await?;
        raw_roots = late_roots;
    }

    let mut root_records = raw_roots.unwrap_or_default();
    if !can_view_codes {
        for node in &mut root_records {
            node.source_sha256 = None;
        }
    }

    let effective_code_limit = if can_view_codes {
        query.code_limit.unwrap_or(limit).min(limit) as usize
    } else {
        0
    };

    let block_roots: Vec<FrontstageBlockNodeResponse> =
        root_records.iter().cloned().map(to_node_response).collect();

    let etag = compute_bootstrap_etag(
        actor,
        can_view_blocks,
        can_view_codes,
        effective_code_limit,
        &pages,
        Some(page_id_str.as_str()),
        resolved_tab_id.as_deref(),
        &tabs,
        page_detail.as_ref(),
        Some(block_roots.as_slice()),
    )?;

    if let Some(condition) = if_none_match {
        if etag_matches(condition, &etag) {
            return Ok(FrontstageBootstrapRead::NotModified(etag));
        }
    }

    let block_codes = if can_view_codes {
        if effective_code_limit == 0 || root_records.is_empty() {
            Some(Vec::new())
        } else {
            let ordered_indices =
                prioritize_viewport_block_indices(&root_records, effective_code_limit);
            let mut unique_refs = Vec::with_capacity(ordered_indices.len());
            let mut seen_refs = HashSet::with_capacity(ordered_indices.len());
            for &idx in &ordered_indices {
                let code_ref = &root_records[idx].code_ref;
                if seen_refs.insert(code_ref.as_str()) {
                    unique_refs.push(code_ref.clone());
                }
            }
            let code_records = service
                .list_block_codes_in_snapshot(
                    &snapshot,
                    workspace_id,
                    page_uuid,
                    tab_uuid,
                    &root_records,
                    &unique_refs,
                )
                .await?;
            let by_ref: HashMap<&str, &domain::frontstage::FrontstageBlockCodeRecord> =
                code_records
                    .iter()
                    .map(|record| (record.code_ref.as_str(), record))
                    .collect();
            let mut responses = Vec::with_capacity(ordered_indices.len());
            for idx in ordered_indices {
                let node = &root_records[idx];
                if let Some(code) = by_ref.get(node.code_ref.as_str()) {
                    responses.push(to_code_response(node.block_id.clone(), (*code).clone()));
                }
            }
            Some(responses)
        }
    } else {
        None
    };

    Ok(FrontstageBootstrapRead::Modified {
        etag,
        payload: Box::new(FrontstageBootstrapResponse {
            pages,
            resolved_page_id: Some(page_id_str),
            resolved_tab_id,
            tabs,
            page_detail,
            block_roots: Some(block_roots),
            block_codes,
        }),
    })
}

fn finish_bootstrap(
    actor: &domain::ActorContext,
    can_view_blocks: bool,
    can_view_codes: bool,
    effective_code_limit: usize,
    if_none_match: Option<&str>,
    payload: FrontstageBootstrapResponse,
) -> Result<FrontstageBootstrapRead, ApiError> {
    let etag = compute_bootstrap_etag(
        actor,
        can_view_blocks,
        can_view_codes,
        effective_code_limit,
        &payload.pages,
        payload.resolved_page_id.as_deref(),
        payload.resolved_tab_id.as_deref(),
        &payload.tabs,
        payload.page_detail.as_ref(),
        payload.block_roots.as_deref(),
    )?;
    if let Some(condition) = if_none_match {
        if etag_matches(condition, &etag) {
            return Ok(FrontstageBootstrapRead::NotModified(etag));
        }
    }
    Ok(FrontstageBootstrapRead::Modified {
        etag,
        payload: Box::new(payload),
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "explicit bootstrap revision envelope"
)]
fn compute_bootstrap_etag(
    actor: &domain::ActorContext,
    can_view_blocks: bool,
    can_view_codes: bool,
    effective_code_limit: usize,
    pages: &[FrontstagePageTreeNodeResponse],
    resolved_page_id: Option<&str>,
    resolved_tab_id: Option<&str>,
    tabs: &[FrontstagePageTabResponse],
    page_detail: Option<&FrontstagePageDetailResponse>,
    block_roots: Option<&[FrontstageBlockNodeResponse]>,
) -> Result<String, ApiError> {
    let serialized = serde_json::to_vec(&(
        "frontstage-bootstrap-v1",
        actor.user_id,
        actor.current_workspace_id,
        &actor.effective_display_role,
        can_view_blocks,
        can_view_codes,
        effective_code_limit,
        pages,
        resolved_page_id,
        resolved_tab_id,
        tabs,
        page_detail,
        block_roots,
    ))
    .map_err(|error| ApiError(anyhow::Error::from(error)))?;
    let digest = Sha256::digest(&serialized);
    Ok(format!("{:x}", base16ct::HexDisplay(&digest)))
}

fn prioritize_viewport_block_indices(
    nodes: &[domain::FrontstageBlockNodeRecord],
    code_limit: usize,
) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..nodes.len()).collect();
    indices.sort_by_key(|&idx| {
        let layout = nodes[idx].runtime_descriptor.get("x-layout");
        let y = extract_layout_coord(layout, "y");
        let x = extract_layout_coord(layout, "x");
        let order = layout
            .and_then(|xl| xl.get("order"))
            .and_then(Value::as_i64)
            .unwrap_or(i64::MAX);
        (y, x, order, idx)
    });
    indices.truncate(code_limit);
    indices
}

fn extract_layout_coord(layout: Option<&Value>, axis: &str) -> i64 {
    let Some(xl) = layout else {
        return i64::MAX;
    };
    for bp in ["lg", "md", "sm"] {
        if let Some(coord) = xl
            .get(bp)
            .and_then(|sub| sub.get(axis))
            .and_then(Value::as_i64)
        {
            return coord;
        }
    }
    xl.get(axis).and_then(Value::as_i64).unwrap_or(i64::MAX)
}

async fn evaluate_block_and_code_permissions(
    deps: &FrontstagePagesDependencies,
    actor: &domain::ActorContext,
) -> Result<(bool, bool), ApiError> {
    if actor.is_root {
        return Ok((true, true));
    }
    let policies = deps
        .store
        .load_console_policy_for_bound_role(
            actor.user_id,
            actor.current_workspace_id,
            &actor.effective_display_role,
        )
        .await?;
    let can_view_blocks = compiled_console_route_access(
        &deps.console_operation_registry,
        "GET",
        "/api/console/frontstage/pages/:page_id/blocks",
    )
    .is_ok_and(|access| authorize_compiled_console_access(&access, actor, &policies));
    let can_view_codes = compiled_console_route_access(
        &deps.console_operation_registry,
        "GET",
        "/api/console/frontstage/pages/:page_id/blocks/:block_id/code",
    )
    .is_ok_and(|access| authorize_compiled_console_access(&access, actor, &policies));
    Ok((can_view_blocks, can_view_codes))
}

fn resolve_target_page_id(
    nodes: &[FrontstagePageTreeNodeResponse],
    query: &FrontstageBootstrapQuery,
) -> Option<String> {
    let normalized_slug = query
        .slug
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let normalized_page_id = query
        .page_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(slug) = normalized_slug {
        let root = nodes.iter().find(|node| {
            matches!(
                node.placement,
                FrontstageNavigationPlacementResponse::Topbar
            ) && node.slug.as_deref() == Some(slug)
        })?;
        return match root.kind {
            FrontstagePageTreeNodeKind::Page => match normalized_page_id {
                None => Some(root.id.clone()),
                Some(pid) if pid == root.id => Some(root.id.clone()),
                Some(_) => None,
            },
            FrontstagePageTreeNodeKind::Group => match normalized_page_id {
                Some(pid) if tree_contains_page(&root.children, pid) => Some(pid.to_owned()),
                Some(_) => None,
                None => first_top_level_or_nested_page_id(&root.children),
            },
        };
    }

    if let Some(pid) = normalized_page_id {
        return tree_contains_page(nodes, pid).then(|| pid.to_owned());
    }

    let root = nodes.iter().find(|node| {
        matches!(
            node.placement,
            FrontstageNavigationPlacementResponse::Topbar
        ) && node
            .slug
            .as_deref()
            .is_some_and(|slug| !slug.trim().is_empty())
    })?;
    match root.kind {
        FrontstagePageTreeNodeKind::Page => Some(root.id.clone()),
        FrontstagePageTreeNodeKind::Group => first_top_level_or_nested_page_id(&root.children),
    }
}

fn first_top_level_or_nested_page_id(nodes: &[FrontstagePageTreeNodeResponse]) -> Option<String> {
    if let Some(top) = nodes
        .iter()
        .find(|node| matches!(node.kind, FrontstagePageTreeNodeKind::Page))
    {
        return Some(top.id.clone());
    }
    for node in nodes {
        if let Some(nested) = first_top_level_or_nested_page_id(&node.children) {
            return Some(nested);
        }
    }
    None
}

fn tree_contains_page(nodes: &[FrontstagePageTreeNodeResponse], target_id: &str) -> bool {
    nodes.iter().any(|node| {
        (matches!(node.kind, FrontstagePageTreeNodeKind::Page) && node.id == target_id)
            || tree_contains_page(&node.children, target_id)
    })
}

fn resolve_target_tab<'a>(
    tabs: &'a [FrontstagePageTabResponse],
    tab_reference: Option<&str>,
) -> Option<&'a FrontstagePageTabResponse> {
    if let Some(reference) = tab_reference.map(str::trim).filter(|v| !v.is_empty()) {
        return tabs
            .iter()
            .find(|tab| tab.id == reference || tab.route_segment.as_deref() == Some(reference));
    }
    let defaults: Vec<&FrontstagePageTabResponse> =
        tabs.iter().filter(|tab| tab.is_default).collect();
    (defaults.len() == 1).then(|| defaults[0])
}
