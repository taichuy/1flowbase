use super::*;

pub(super) fn core_openapi_operation_ids() -> anyhow::Result<BTreeMap<(String, String), String>> {
    let document = serde_json::to_value(crate::openapi::ApiDoc::openapi())?;
    let paths = document["paths"]
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("OpenAPI paths are missing"))?;
    let mut operation_ids = BTreeMap::new();
    for (path, item) in paths {
        let Some(methods) = item.as_object() else {
            continue;
        };
        for (method, operation) in methods {
            if !matches!(method.as_str(), "get" | "post" | "put" | "patch" | "delete") {
                continue;
            }
            let operation_id = operation["operationId"].as_str().ok_or_else(|| {
                anyhow::anyhow!("OpenAPI operationId is missing for {method} {path}")
            })?;
            let key = (method.to_ascii_uppercase(), route_identity_shape(path));
            if operation_ids
                .insert(key, operation_id.to_string())
                .is_some()
            {
                anyhow::bail!("duplicate OpenAPI interface identity for {method} {path}");
            }
        }
    }
    // These internal console interfaces are mounted by the same typed route assembly but are not
    // published in the public OpenAPI document. Their IDs are explicit and stable; adding an
    // OpenAPI entry later must preserve the ID before this fallback is removed.
    for (method, path, operation_id) in [
        (
            "DELETE",
            "/api/console/mcp/instances/{}/client-credential",
            "delete_mcp_instance_client_credential",
        ),
        (
            "DELETE",
            "/api/console/settings/files/storages/{}",
            "delete_file_storage",
        ),
        (
            "DELETE",
            "/api/console/settings/files/tables/{}",
            "delete_file_table",
        ),
        (
            "GET",
            "/api/console/applications/{}/logs/conversations/{}/messages",
            "list_application_conversation_messages",
        ),
        (
            "GET",
            "/api/console/applications/{}/logs/runs/{}/conversation/messages",
            "list_application_run_conversation_messages",
        ),
        (
            "GET",
            "/api/console/docs/catalog",
            "get_console_docs_catalog",
        ),
        (
            "GET",
            "/api/console/docs/categories/{}/openapi.json",
            "get_console_docs_category_openapi",
        ),
        (
            "GET",
            "/api/console/docs/categories/{}/operations",
            "list_console_docs_category_operations",
        ),
        (
            "GET",
            "/api/console/docs/operations/{}/openapi.json",
            "get_console_docs_operation_openapi",
        ),
        (
            "GET",
            "/api/console/mcp/bundles/export-defaults",
            "get_mcp_bundle_export_defaults",
        ),
        (
            "GET",
            "/api/console/mcp/bundles/official",
            "list_official_mcp_bundles",
        ),
        (
            "GET",
            "/api/console/mcp/bundles/library",
            "list_mcp_bundle_library",
        ),
        (
            "GET",
            "/api/console/mcp/instances/{}/client-credential",
            "get_mcp_instance_client_credential",
        ),
        (
            "GET",
            "/api/console/model-providers/providers/{}/icon",
            "get_model_provider_icon",
        ),
        (
            "POST",
            "/api/console/frontstage/pages/{}/tabs/{}/actions/dispatch",
            "dispatch_frontstage_tab_action",
        ),
        (
            "POST",
            "/api/console/frontstage/pages/{}/tabs/{}/queries/dispatch",
            "dispatch_frontstage_tab_query",
        ),
        (
            "GET",
            "/api/console/settings/members/role-options",
            "list_member_role_options",
        ),
        (
            "GET",
            "/api/console/settings/roles/{}/console-policy",
            "get_role_console_policy",
        ),
        (
            "GET",
            "/api/console/settings/roles/{}/frontstage-routes",
            "get_role_frontstage_routes",
        ),
        (
            "GET",
            "/api/console/settings/roles/console-policy-catalog",
            "get_console_policy_catalog",
        ),
        (
            "POST",
            "/api/console/mcp/bundles/export",
            "export_mcp_bundle",
        ),
        (
            "POST",
            "/api/console/mcp/bundles/import-official",
            "import_official_mcp_bundle",
        ),
        (
            "POST",
            "/api/console/mcp/bundles/import-upload",
            "import_uploaded_mcp_bundle",
        ),
        (
            "POST",
            "/api/console/mcp/bundles/preview-official",
            "preview_official_mcp_bundle",
        ),
        (
            "POST",
            "/api/console/mcp/bundles/preview-upload",
            "preview_uploaded_mcp_bundle",
        ),
        (
            "POST",
            "/api/console/mcp/bundles/library/{}/{}/sync",
            "sync_mcp_bundle_library_release",
        ),
        (
            "POST",
            "/api/console/mcp/bundles/library/{}/{}/preview",
            "preview_mcp_bundle_library_release",
        ),
        (
            "POST",
            "/api/console/mcp/bundles/library/{}/{}/import",
            "import_mcp_bundle_library_release",
        ),
        (
            "POST",
            "/api/console/mcp/bundles/library/{}/{}/current/{}",
            "switch_mcp_bundle_library_release",
        ),
        (
            "DELETE",
            "/api/console/mcp/bundles/library/{}/{}/releases/{}",
            "delete_mcp_bundle_library_release",
        ),
        (
            "POST",
            "/api/console/mcp/bundles/library/{}/{}/releases/{}/repair",
            "repair_mcp_bundle_library_release",
        ),
        (
            "POST",
            "/api/console/mcp/instances/{}/bundles/export",
            "export_mcp_instance_bundle",
        ),
        (
            "POST",
            "/api/console/mcp/instances/{}/groups/move",
            "move_mcp_group",
        ),
        (
            "POST",
            "/api/console/plugins/{}/artifact/install-current-node",
            "install_plugin_artifact_on_current_node",
        ),
        (
            "POST",
            "/api/console/plugins/{}/artifact/refresh",
            "refresh_plugin_artifact",
        ),
        (
            "PUT",
            "/api/console/mcp/instances/{}/client-credential",
            "replace_mcp_instance_client_credential",
        ),
        (
            "PUT",
            "/api/console/settings/files/storages/{}",
            "update_file_storage",
        ),
        (
            "PUT",
            "/api/console/settings/roles/{}/console-policy",
            "replace_role_console_policy",
        ),
        (
            "PUT",
            "/api/console/settings/roles/{}/frontstage-routes",
            "replace_role_frontstage_routes",
        ),
    ] {
        operation_ids
            .entry((method.to_string(), path.to_string()))
            .or_insert_with(|| operation_id.to_string());
    }
    Ok(operation_ids)
}

pub(super) fn expand_core_interface_registrations(
    registrations: Vec<ConsoleOperationRegistration>,
) -> anyhow::Result<Vec<ConsoleOperationRegistration>> {
    let openapi_ids = core_openapi_operation_ids()?;
    let mut expanded = Vec::new();
    let mut missing = Vec::new();
    for registration in registrations {
        if registration.authorization_profile_id.is_none() || registration.routes.len() <= 1 {
            expanded.push(registration);
            continue;
        }
        for (route_order, route) in registration.routes.iter().enumerate() {
            let key = (
                route.method.to_ascii_uppercase(),
                route_identity_shape(&route.path),
            );
            let Some(operation_id) = openapi_ids.get(&key) else {
                missing.push(format!("{} {}", route.method, route.path));
                continue;
            };
            let mut interface = registration.clone();
            interface.authorization_profile_id = Some(registration.operation_id.clone());
            interface.operation_id = operation_id.clone();
            interface.order = registration.order * 1000 + route_order as i32;
            interface.routes = vec![route.clone()];
            expanded.push(interface);
        }
    }
    if !missing.is_empty() {
        missing.sort();
        anyhow::bail!(
            "configurable console routes have no stable OpenAPI operationId: {}",
            missing.join(", ")
        );
    }
    Ok(expanded)
}

pub(super) fn static_english_interface_summary(interface_id: &str) -> String {
    let owned_summary = match interface_id {
        "local_commands.execute" => Some("Execute one local shell command"),
        "communications.options.get" => Some("List communication execution options"),
        "communications.binding.get" => Some("Read communication binding"),
        "communications.binding.update" => Some("Save communication binding"),
        "communications.conversations.list" => Some("List communication conversations"),
        "communications.management.conversations.messages.list" => Some("Read chronological conversation messages"),
        "communications.management.conversations.list" => Some("List workspace communication conversations"),
        "communications.management.messages.list" => Some("List workspace received messages"),
        "communications.management.messages.get" => Some("Read a workspace received message"),
        "communications.messages.list" => Some("Read communication history"),
        "communications.files.list" => Some("List received communication files"),
        "communications.files.get" => Some("Read a received communication file"),
        "communications.messages.retry" => Some("Retry failed communication processing"),
        "notifications.logs.list" => Some("List notification send logs"),
        "notifications.logs.get" => Some("Get notification send log"),
        "notifications.channels.list" => Some("List my notification channels"),
        "notifications.channels.read_all" => Some("Read my notification channel"),
        "notifications.messages.unread" => Some("Mark my notification unread"),
        "notifications.types.list" => Some("List notification types"),
        "notifications.instances.list" => Some("List notification instances"),
        "notifications.instances.get" => Some("Get notification instance"),
        "notifications.instances.create" => Some("Create notification instance"),
        "notifications.instances.update" => Some("Update notification instance"),
        "notifications.instances.delete" => Some("Delete notification instance"),
        "notifications.instances.send" => Some("Send notification"),
        "notifications.instances.authorization" => Some("Authorize notification instance"),
        "notifications.messages.list" => Some("List my notification messages"),
        "notifications.messages.unread_count" => Some("Count my unread notifications"),
        "notifications.messages.read" => Some("Read my notification"),
        "notifications.messages.read_all" => Some("Read all my notifications"),
        "notifications.events.subscribe" => Some("Subscribe to notification refresh signals"),

        "files.access_links.create" => Some("Create a temporary public file link"),
        "applications.logs.otlp.export" => Some("OTLP JSON file"),
        "applications.logs.otlp.import" => Some("OTLP JSON facts"),
        "applications.logs.delete" => Some("Delete imported agent log records"),
        "applications.logs.delete.preview" => Some("Count imported logs in a deletion scope"),
        "applications.logs.delete.jobs.create" => Some("Start a persistent log deletion"),
        "applications.logs.delete.jobs.get" => Some("Read log deletion progress"),
        "applications.logs.delete.jobs.latest" => Some("Find the latest log deletion"),
        "applications.logs.delete.jobs.stop" => Some("Stop a log deletion after its current batch"),

        "get_current_run_context" => Some("Read the current MCP caller turn and log record identity"),
        "applications.runtime.records.query-fields" => Some("Discover source-neutral log query fields"),
        "applications.runtime.records.query" => Some("Query source-neutral log records"),
        "applications.runtime.conversations.query" => Some("Query canonical log conversations"),
        "applications.runtime.conversation-trajectory.query" => Some("Query the canonical conversation timeline including subagents"),
        "applications.runtime.conversation-trajectory.section" => Some("Read a canonical conversation event section"),
        "applications.runtime.record.trace-tree.get" => Some("Read the selected log record trace root"),
        "applications.runtime.record.trace-tree.children.list" => Some("Page direct children of a log record trace node"),
        "applications.runtime.record.client-trajectory.query" => Some("Filter recorded client trajectory and locate semantic keyword hits"),
        "applications.runtime.record.conversation.messages.list" | "list_record_conversation_messages" => Some("List source-neutral business turns around a selected log record"),
        "applications.runtime.record.get" => Some("Get source-neutral log record"),
        "applications.runtime.record.client-trajectory.list" => Some("List recorded client trajectory"),
        "applications.runtime.record.client-trajectory.section.get" => Some("Get recorded client trajectory section"),
        "console.departments.access" => Some("Read organization operation access"),
        "departments.list" => Some("List workspace departments"),
        "departments.create" => Some("Create a workspace department"),
        "departments.update" => Some("Update a workspace department"),
        "departments.delete" => Some("Delete an empty workspace department"),
        "members.departments.replace" => Some("Replace member department assignments"),
        "system_templates.catalog" => Some("List portable template resources"),
        "system_templates.export" => Some("Export selected portable definitions"),
        "system_templates.preview" => Some("Preview portable template installation"),
        "system_templates.install" => Some("Install portable template resources"),

        "network_egress_proxies.get" => Some("Read proxy configuration for editing"),
        "network_egress_proxies.update" => Some("Update proxy configuration"),
        "billing.pricing_catalog.sync" => Some("Synchronize all official model prices"),
        "extension_center.installed.delete" => Some(
            "Remove an installed extension artifact; runtime and capability plugins unload their family while preserving durable data",
        ),
        "extension_center.installed.disable" => Some("Disable an installed executable extension"),
        "extension_center.contribution_authorizations.grant" => Some("Grant exact installed contribution permissions in the current workspace"),
        "extension_center.contribution_authorizations.revoke" => Some("Revoke exact installed contribution permissions in the current workspace"),
        "extension_center.managed_execution.view" => Some("View managed executions and durable lifecycle deliveries in the current workspace"),
        "extension_center.lifecycle_deliveries.resume" => Some("Resume an exact paused lifecycle delivery after current authority and original executable validation"),
        "extension_center.managed_executions.retire" => Some("Retire a frozen managed execution only after publication, invocation and durable delivery references are gone"),
        "extension_center.contribution_authorizations.view" => Some("View exact installed contribution permissions in the current workspace"),
        "extension_center.installed.enable" => Some("Enable an installed executable extension"),
        "extension_center.installed.select" => Some("Select an installed extension version"),
        "mcp.bundle_library.current.switch" => Some("Switch MCP bundle library release"),
        "mcp.bundle_library.import" => Some("Import MCP bundle library release"),
        "mcp.bundle_library.preview" => Some("Preview MCP bundle library release"),
        "mcp.bundle_library.releases.delete" => Some("Delete MCP bundle library release"),
        "mcp.bundle_library.releases.repair" => Some("Repair MCP bundle library release"),
        "mcp.bundle_library.sync" => Some("Sync MCP bundle library release"),
        _ => None,
    };
    if let Some(summary) = owned_summary {
        return summary.to_string();
    }

    const ACTIONS: &[&str] = &[
        "get",
        "list",
        "create",
        "update",
        "replace",
        "delete",
        "patch",
        "revoke",
        "enable",
        "disable",
        "copy",
        "move",
        "export",
        "import",
        "preview",
        "refresh",
        "install",
        "publish",
        "unpublish",
        "restore",
        "start",
        "cancel",
        "complete",
        "resume",
        "resolve",
        "search",
        "save",
        "test",
        "discover",
        "execute",
        "clear",
        "reveal",
        "reset",
        "upload",
        "upsert",
        "bind",
        "check",
        "subscribe",
        "manage",
        "view",
    ];
    let mut words = interface_id
        .split(['.', '_', '-'])
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    if words.len() > 1
        && !ACTIONS.contains(&words[0])
        && words.last().is_some_and(|word| ACTIONS.contains(word))
    {
        let action = words.pop().expect("non-empty interface words");
        words.insert(0, action);
    }
    let mut summary = words
        .into_iter()
        .map(|word| match word {
            "api" => "API".to_string(),
            "apis" => "APIs".to_string(),
            "id" => "ID".to_string(),
            "ids" => "IDs".to_string(),
            "js" => "JS".to_string(),
            "llm" => "LLM".to_string(),
            "mcp" => "MCP".to_string(),
            "ui" => "UI".to_string(),
            "url" => "URL".to_string(),
            _ => word.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ");
    if let Some(first) = summary.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    summary
}

pub(super) fn compile_console_interface_metadata(
    bindings: &[ConsoleRouteAssemblyBinding],
    registry: &ConsoleOperationRegistry,
) -> anyhow::Result<Vec<ConsoleInterfaceRegistration>> {
    let openapi_ids = core_openapi_operation_ids()?;
    let mut missing = Vec::new();
    let mut interfaces = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let operation = registry.inventory().operations.iter().find(|operation| {
            operation.routes.iter().any(|route| {
                route.method.eq_ignore_ascii_case(&binding.route.method)
                    && route_templates_match(&route.path, &binding.route.path)
            })
        });
        let interface_id = match (&binding.ownership, operation) {
            (ConsoleRouteOwnership::ConsoleOperation(_), Some(operation)) => {
                operation.operation_id.clone()
            }
            (ConsoleRouteOwnership::Authenticated, Some(_)) => {
                let key = (
                    binding.route.method.to_ascii_uppercase(),
                    route_identity_shape(&binding.route.path),
                );
                let Some(interface_id) = openapi_ids.get(&key) else {
                    missing.push(format!("{} {}", binding.route.method, binding.route.path));
                    continue;
                };
                interface_id.clone()
            }
            (_, None) => {
                missing.push(format!("{} {}", binding.route.method, binding.route.path));
                continue;
            }
        };
        let summary = match interface_id.as_str() {
            "system.branding.get" => "Read system settings".to_owned(),
            "system.branding.save" => "Save system settings".to_owned(),
            _ => static_english_interface_summary(&interface_id),
        };
        let description = match interface_id.as_str() {
            "local_commands.execute" => "Execute a single command string in the backend instance environment as its OS account. Requires the local command operation permission; returns complete output and termination status without a reusable session, deployment opt-in, or automatic retry.".to_owned(),
            "communications.options.get" => "Return published Agent-flow applications, visible enabled MCP instances and the selected application model and reasoning capabilities.".to_owned(),
            "communications.binding.get" => "Read the channel execution binding and plugin-owned default prompt without credentials.".to_owned(),
            "communications.binding.update" => "Validate and store the channel application, model, reasoning, file table, editable system prompt and selected MCP instances.".to_owned(),
            "communications.conversations.list" => "Page channel conversations in the current workspace; delegated communication runs are restricted to their external identity.".to_owned(),
            "communications.management.conversations.messages.list" => "Read the latest five typed messages, with a scoped cursor for older history. Delegated execution is denied.".to_owned(),
            "communications.management.conversations.list" => "Page conversations across workspace channels with base conversation identity fields. Requires communication management permission; delegated channel execution is denied.".to_owned(),
            "communications.management.messages.list" => "Page received message summaries across workspace channels with channel, conversation and status filters. Requires communication management permission; delegated channel execution is denied.".to_owned(),
            "communications.management.messages.get" => "Read one typed message and its original run application in the current workspace. Requires communication management permission; delegated channel execution is denied.".to_owned(),
            "communications.messages.list" => "Page received messages and final replies for a channel conversation within the authenticated or delegated data scope.".to_owned(),
            "communications.files.list" => "Page safe file metadata associated with messages in an authorized channel conversation; omit provider download credentials.".to_owned(),
            "communications.files.get" => "Authorize attachment ownership and file access, then read one bounded content range with stable authenticated file links.".to_owned(),
            "communications.messages.retry" => "Retry a failed message or explicitly rejected send only when backend eligibility permits; unknown sends cannot be retried automatically.".to_owned(),
            "notifications.logs.list" => "Page safe send invocation facts in the current workspace using explicit log-read authorization.".to_owned(),
            "notifications.logs.get" => "Read one safe send invocation fact in the current workspace using explicit log-detail authorization.".to_owned(),
            "notifications.channels.list" => "Page channels from the authenticated recipient inbox with filtered latest messages and independently aggregated unread counts.".to_owned(),
            "notifications.channels.read_all" => "Mark unread inbox messages read only in the requested channel, current workspace and authenticated recipient.".to_owned(),
            "notifications.messages.unread" => "Clear the read timestamp only for a message owned by the authenticated recipient in the current workspace.".to_owned(),
            "notifications.types.list" => "Return canonical notification types and valid recipient form options available in the current workspace.".to_owned(),
            "notifications.instances.list" => "List workspace notification instances with credential fields removed and explicit availability status.".to_owned(),
            "notifications.instances.get" => "Read one instance in the current workspace with stored credentials omitted.".to_owned(),
            "notifications.instances.create" => "Validate the type configuration and store one workspace instance with password fields encrypted.".to_owned(),
            "notifications.instances.update" => "Update instance name, status or configuration in its workspace while retaining omitted password fields.".to_owned(),
            "notifications.instances.delete" => "Delete one workspace notification instance and preserve independently stored inbox history.".to_owned(),
            "notifications.instances.authorization" => "Begin, poll, cancel or maintain provider authorization for one current-workspace notification instance; return only the public authorization view and persist managed credentials encrypted.".to_owned(),
            "notifications.instances.send" => "Send validated invocation input through an enabled workspace instance and return the committed or provider-confirmed result.".to_owned(),
            "notifications.messages.list" => "Page only the authenticated recipient inbox in the current workspace and return its unread count.".to_owned(),
            "notifications.messages.unread_count" => "Count only unread messages owned by the authenticated recipient in the current workspace.".to_owned(),
            "notifications.messages.read" => "Mark only the authenticated recipient message read, preserving the original read timestamp on repeated calls.".to_owned(),
            "notifications.messages.read_all" => "Mark all unread messages of the authenticated recipient in the current workspace read.".to_owned(),
            "notifications.events.subscribe" => "Open an authenticated recipient-scoped protocol stream carrying only inbox refresh signals; reconnect readers load database facts.".to_owned(),

            "files.access_links.create" => "Check the current caller file record read permission and create an ephemeral, time-limited link usable without login. Does not modify the stored stable access path.".to_owned(),
            "get_current_run_context" => "Resolve client-declared MCP thread and turn metadata against readable log applications in the current workspace. Return explicit missing, invalid, conflicting, absent or ambiguous states without guessing a run identity.".to_owned(),
            "applications.runtime.record.conversation.messages.list" | "list_record_conversation_messages" => "Read the latest five main-session turns initially. Normalize child anchors to their canonical main conversation. Page older or newer main turns with before/after cursors bound to this authorized conversation; child and foreign cursors are rejected. Return projected system/user/final messages and each real record trajectory locator. Subagent events are available through the canonical conversation timeline with their original provenance.".to_owned(),
            "applications.runtime.conversations.query" => "Query canonical conversations in readable applications of the current workspace. Filter main task facts, aggregate each member task once, and return exact totals with numbered paging and the latest main record locator. Sort only declared fields.".to_owned(),
            "applications.runtime.record.trace-tree.get" => "Read one selected task root as trace metadata with explicit record_id, parent identity, own usage and lazy child counts. No transcripts, raw bodies or fabricated native flow identities.".to_owned(),
            "applications.runtime.record.trace-tree.children.list" => "Page direct child metadata for a descendant of the selected record in the same source namespace. Cursors are bound to the selected record and node. Load content through each child's explicit record_id.".to_owned(),
            "applications.runtime.conversation-trajectory.query" => "Read timestamp-ordered original events of one canonical imported conversation, including all owned subagents. Filter original category, time and provenance or search restored semantic sections across the whole conversation. Opaque previous/next cursors bind the query and authorized scope; focus_record_id positions without narrowing membership. Restart without a cursor to refresh late history.".to_owned(),
            "applications.runtime.conversation-trajectory.section" => "Authorize the original step through its application, scope, source namespace and canonical conversation, then lazily read the selected section from its owning record. Preserves raw source facts and rejects foreign step identifiers.".to_owned(),
            "applications.runtime.records.query-fields" => "Return typed record and trajectory filter fields, operators, sort fields, keyword scopes and cursor semantics.".to_owned(),
            "applications.runtime.records.query" => "Query Native and imported task snapshots in the current workspace using authorized application selection, AND/OR field predicates, optional title/input/final-output SQL contains and stable keyset paging. No exact count or raw archive search.".to_owned(),
            "applications.runtime.record.client-trajectory.query" => "Filter steps of an authorized log record, restore explicitly selected semantic section bodies for substring search, and return real step/section/sequence hit locators. Raw protocol sections remain available through the section reader.".to_owned(),
            "applications.logs.otlp.export" => "Export complete native AgentFlow or imported AgentLogs source facts as standard OTLP JSON using an independent Console operation. Exactly one nonempty record or conversation selector is required. Historical export does not require reception enabled.".to_owned(),
            "applications.logs.otlp.import" => "Import standard OTLP JSON source facts into an AgentLogs application using an independent Console operation and enabled reception. Preserve source identity and truthful partial-commit receipts; never create executable flow runs.".to_owned(),
            "applications.logs.delete" => "Delete complete imported turns from an agent_logs application only, using explicit all_time mode or a half-open [started_at_from,started_at_to) RFC3339 time_range. Optional positive batch_size limits each transaction; omit it for full-scope atomic deletion. Repeat while has_more with the first receipt ingested_at_before to exclude later imports. Preserve the application, keys, credits and other applications.".to_owned(),
            "console.departments.access" => "Return current organization operation eligibility from effective workspace role policies.".to_owned(),
            "departments.list" => "List departments with parent IDs, assigned role codes and distinct subtree member counts in the current workspace.".to_owned(),
            "departments.create" => "Create a department using the ordered tree contract; assigning roles requires member role assignment eligibility.".to_owned(),
            "departments.update" => "Update department name, parent and role assignments while rejecting cycles and cross-workspace references.".to_owned(),
            "departments.delete" => "Delete a department only when it has no children and no assigned members.".to_owned(),
            "members.departments.replace" => "Atomically replace a member's department assignments and require exactly one primary department when assignments exist.".to_owned(),
            "network_egress_proxies.get" => "Return public proxy configuration and configured secret field names without revealing stored credentials.".to_string(),
            "network_egress_proxies.update" => "Update the existing proxy configuration, preserve omitted secrets and keep member and route identities. The provider type cannot change.".to_string(),
            "billing.pricing_catalog.sync" => "Fetch the complete official catalog and atomically update official prices and conditional rules, insert new models, and retire duplicate official records while preserving manual rules and historical IDs.".to_string(),
            "extension_center.managed_execution.view" => "Read exact retained and current execution targets, frozen reference counts and durable delivery status for the installed extension in the current workspace.".to_string(),
            "extension_center.lifecycle_deliveries.resume" => "Resume one paused delivery only when its full frozen target, original executable and current contribution authorization remain valid.".to_string(),
            "extension_center.managed_executions.retire" => "Reject retirement while the target is current or retains frozen invocations, future publications, admitted work or unfinished durable deliveries; preserve every delivery record.".to_string(),
            "system.branding.get" => "Read installation-wide logo, system name, login tagline visibility, description and filing HTML.".to_owned(),
            "system.branding.save" => "Replace installation-wide public branding. Store administrator-authored filing HTML verbatim.".to_owned(),
            _ => format!("{summary} in the system backend."),
        };
        let external_auth_metadata = super::super::external_auth::interface::bindings_metadata()
            .iter()
            .find(|metadata| {
                metadata.method.eq_ignore_ascii_case(&binding.route.method)
                    && metadata.path == binding.route.path
            });
        let methods_metadata = super::super::me_authentication_methods::interface::metadata(
            &binding.route.method,
            &binding.route.path,
        );
        interfaces.push(ConsoleInterfaceRegistration {
            interface_id,
            route: binding.route.clone(),
            description: methods_metadata
                .map(|(_, description)| description.to_owned())
                .or_else(|| external_auth_metadata.map(|metadata| metadata.description.to_owned()))
                .unwrap_or(description),
            summary: methods_metadata
                .map(|(summary, _)| summary.to_owned())
                .or_else(|| external_auth_metadata.map(|metadata| metadata.summary.to_owned()))
                .unwrap_or(summary),
        });
    }
    if !missing.is_empty() {
        missing.sort();
        anyhow::bail!(
            "console routes have no static interface identity: {}",
            missing.join(", ")
        );
    }
    Ok(interfaces)
}
