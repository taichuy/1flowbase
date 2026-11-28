use super::*;

pub fn infrastructure_mcp_and_user_api_key_routes_compile_exact_operations() {
    let host = crate::routes::host_infrastructure::route_assembly();
    let mcp = crate::routes::mcp_management::route_assembly();
    let user_api_keys = crate::routes::user_api_keys::route_assembly();

    assert_eq!(
        route_bindings(&host),
        vec![
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory",
                "host_infrastructure.memory.view"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/stats",
                "host_infrastructure.memory.view"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/entries",
                "host_infrastructure.memory.view"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/stats",
                "host_infrastructure.memory.view"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/entries/search",
                "host_infrastructure.memory.view"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/tree",
                "host_infrastructure.memory.view"
            ),
            (
                "POST",
                "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/entries/reveal",
                "host_infrastructure.memory.reveal"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/cache",
                "host_infrastructure.cache.view"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/cache/domains/:domain_code/entries",
                "host_infrastructure.cache.view"
            ),
            (
                "POST",
                "/api/console/settings/host-infrastructure/cache/domains/:domain_code/entries/reveal",
                "host_infrastructure.cache.reveal"
            ),
            (
                "POST",
                "/api/console/settings/host-infrastructure/cache/domains/:domain_code/entries/clear",
                "host_infrastructure.cache.entry.clear"
            ),
            (
                "POST",
                "/api/console/settings/host-infrastructure/cache/domains/:domain_code/clear",
                "host_infrastructure.cache.domain.clear"
            ),
        ]
    );
    assert_eq!(
        route_bindings(&mcp),
        vec![
            ("GET", "/api/console/mcp/catalog", "mcp.catalog.view"),
            (
                "GET",
                "/api/console/mcp/interface-capabilities",
                "mcp.catalog.view"
            ),
            ("GET", "/api/console/mcp/interface-capabilities/page", "mcp.catalog.view"),
            ("GET", "/api/console/mcp/interface-capabilities/:interface_id", "mcp.catalog.view"),
            ("GET", "/api/console/mcp/list", "mcp.catalog.view"),
            ("GET", "/api/console/mcp/export", "mcp.catalog.export"),
            ("GET", "/api/console/mcp/instances", "mcp.instances.view"),
            ("POST", "/api/console/mcp/instances", "mcp.instances.create"),
            ("GET", "/api/console/mcp/instances/page", "mcp.instances.view"),
            (
                "POST",
                "/api/console/mcp/instances/:instance_id/copy",
                "mcp.instances.copy"
            ),
            (
                "PUT",
                "/api/console/mcp/instances/:instance_id",
                "mcp.instances.update"
            ),
            (
                "DELETE",
                "/api/console/mcp/instances/:instance_id",
                "mcp.instances.delete"
            ),
            (
                "GET",
                "/api/console/mcp/instances/:instance_id/client-credential",
                "mcp.client_credential.reveal"
            ),
            (
                "PUT",
                "/api/console/mcp/instances/:instance_id/client-credential",
                "mcp.client_credential.save"
            ),
            (
                "DELETE",
                "/api/console/mcp/instances/:instance_id/client-credential",
                "mcp.client_credential.delete"
            ),
            (
                "GET",
                "/api/console/mcp/instances/:instance_id/directory",
                "mcp.directory.view"
            ),
            (
                "PUT",
                "/api/console/mcp/instances/:instance_id/directory",
                "mcp.directory.replace"
            ),
            (
                "POST",
                "/api/console/mcp/instances/:instance_id/groups",
                "mcp.groups.upsert"
            ),
            (
                "DELETE",
                "/api/console/mcp/instances/:instance_id/groups",
                "mcp.groups.delete"
            ),
            (
                "POST",
                "/api/console/mcp/instances/:instance_id/groups/move",
                "mcp.groups.move"
            ),
            (
                "POST",
                "/api/console/mcp/instances/:instance_id/tool-bindings",
                "mcp.tool_bindings.create"
            ),
            (
                "PUT",
                "/api/console/mcp/tool-bindings/:binding_id",
                "mcp.tool_bindings.update"
            ),
            (
                "DELETE",
                "/api/console/mcp/tool-bindings/:binding_id",
                "mcp.tool_bindings.delete"
            ),
            ("GET", "/api/console/mcp/tools", "mcp.tools.view"),
            ("POST", "/api/console/mcp/tools", "mcp.tools.create"),
            ("GET", "/api/console/mcp/tools/page", "mcp.tools.view"),
            ("GET", "/api/console/mcp/tools/:tool_id", "mcp.tools.view"),
            ("PUT", "/api/console/mcp/tools/:tool_id", "mcp.tools.update"),
            (
                "DELETE",
                "/api/console/mcp/tools/:tool_id",
                "mcp.tools.delete"
            ),
            (
                "POST",
                "/api/console/mcp/tools/:tool_id/description/refresh",
                "mcp.tools.description.refresh"
            ),
            (
                "POST",
                "/api/console/mcp/tools/:tool_id/description-check",
                "mcp.tools.description.check"
            ),
            (
                "POST",
                "/api/console/mcp/debug/execute",
                "mcp.debug.execute"
            ),
            (
                "GET",
                "/api/console/mcp/instances/:instance_id/discovery-policy",
                "mcp.discovery_policy.view"
            ),
            (
                "PUT",
                "/api/console/mcp/instances/:instance_id/discovery-policy",
                "mcp.discovery_policy.update"
            ),
            (
                "GET",
                "/api/console/mcp/bundles/official",
                "mcp.bundles.official.list"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/preview-official",
                "mcp.bundles.preview"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/import-official",
                "mcp.bundles.import"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/export",
                "mcp.bundles.export"
            ),
            (
                "GET",
                "/api/console/mcp/bundles/export-defaults",
                "mcp.bundles.export"
            ),
            (
                "POST",
                "/api/console/mcp/instances/:instance_id/bundles/export",
                "mcp.instances.export"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/preview-upload",
                "mcp.bundles.preview"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/import-upload",
                "mcp.bundles.import"
            ),
            (
                "GET",
                "/api/console/mcp/bundles/library",
                "mcp.bundle_library.list"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/library/:organization/:bundle_id/sync",
                "mcp.bundle_library.sync"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/library/:organization/:bundle_id/preview",
                "mcp.bundle_library.preview"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/library/:organization/:bundle_id/import",
                "mcp.bundle_library.import"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/library/:organization/:bundle_id/current/:bundle_version",
                "mcp.bundle_library.current.switch"
            ),
            (
                "DELETE",
                "/api/console/mcp/bundles/library/:organization/:bundle_id/releases/:bundle_version",
                "mcp.bundle_library.releases.delete"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/library/:organization/:bundle_id/releases/:bundle_version/repair",
                "mcp.bundle_library.releases.repair"
            ),
            (
                "GET",
                "/api/console/mcp/upstream-connections",
                "mcp.upstream_connections.view"
            ),
            (
                "POST",
                "/api/console/mcp/upstream-connections",
                "mcp.upstream_connections.create"
            ),
            (
                "PUT",
                "/api/console/mcp/upstream-connections/:connection_id",
                "mcp.upstream_connections.update"
            ),
            (
                "DELETE",
                "/api/console/mcp/upstream-connections/:connection_id",
                "mcp.upstream_connections.delete"
            ),
            (
                "PUT",
                "/api/console/mcp/upstream-connections/:connection_id/credentials",
                "mcp.upstream_credentials.update"
            ),
            (
                "DELETE",
                "/api/console/mcp/upstream-connections/:connection_id/credentials",
                "mcp.upstream_credentials.delete"
            ),
            (
                "POST",
                "/api/console/mcp/upstream-connections/test",
                "mcp.upstream_connections.test"
            ),
            (
                "POST",
                "/api/console/mcp/upstream-connections/:connection_id/test",
                "mcp.upstream_connections.test"
            ),
            (
                "POST",
                "/api/console/mcp/upstream-connections/:connection_id/discover",
                "mcp.upstream_connections.discover"
            ),
            (
                "POST",
                "/api/console/mcp/upstream-connections/:connection_id/imports",
                "mcp.upstream_tools.import"
            ),
            (
                "POST",
                "/api/console/mcp/tools/:tool_id/debug",
                "mcp.upstream_tools.debug"
            ),
        ]
    );
    let settings = crate::app_state::compile_core_settings_feature_registry().unwrap();
    let bindings = crate::routes::console_route_assembly::migrated_core_console_contract_bindings();
    let registry =
        crate::routes::console_route_assembly::compile_migrated_core_console_operation_registry(
            &settings, &bindings,
        )
        .unwrap();
    for (path, operation_id, profile_id) in [
        ("instances", "mcp.instances.view", "mcp.instances.view"),
        (
            "instances/page",
            "list_mcp_instances_page",
            "mcp.instances.view",
        ),
        ("tools/page", "list_mcp_tools_page", "mcp.tools.view"),
        ("tools/a-tool", "get_mcp_tool", "mcp.tools.view"),
        ("tools/by-id", "get_mcp_tool", "mcp.tools.view"),
        (
            "interface-capabilities/page",
            "list_mcp_interface_capabilities_page",
            "mcp.catalog.view",
        ),
        (
            "interface-capabilities/an-interface",
            "get_mcp_interface_capability",
            "mcp.catalog.view",
        ),
        (
            "interface-capabilities/by-id",
            "get_mcp_interface_capability",
            "mcp.catalog.view",
        ),
    ] {
        let path = format!("/api/console/mcp/{path}");
        let access = registry.access_for_console_route("GET", &path).unwrap();
        assert_eq!(access.operation_id, operation_id, "{path}");
        let operation = registry
            .inventory()
            .operations
            .iter()
            .find(|operation| operation.operation_id == access.operation_id)
            .unwrap();
        assert_eq!(operation.authorization_profile_id, profile_id, "{path}");
    }
    for (method, path, operation_id) in [
        ("PUT", "tools/page", "mcp.tools.update"),
        ("DELETE", "tools/page", "mcp.tools.delete"),
        ("PUT", "instances/page", "mcp.instances.update"),
        ("DELETE", "instances/page", "mcp.instances.delete"),
    ] {
        let access = registry
            .access_for_console_route(method, &format!("/api/console/mcp/{path}"))
            .unwrap();
        assert_eq!(access.operation_id, operation_id);
    }
    assert_eq!(
        route_bindings(&user_api_keys),
        vec![
            ("GET", "/api/console/user-api-keys", "user_api_keys.manage"),
            ("POST", "/api/console/user-api-keys", "user_api_keys.manage"),
            (
                "GET",
                "/api/console/user-api-keys/role-options",
                "user_api_keys.manage"
            ),
            (
                "POST",
                "/api/console/user-api-keys/:api_key_id/revoke",
                "user_api_keys.manage"
            ),
        ]
    );
}
