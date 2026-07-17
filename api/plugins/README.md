# API Plugin Packages

## Workspace Layout

```text
api/plugins/
  host-extensions/<extension_id>/
  runtime-extensions/<plugin_id>/
  capability-plugins/<plugin_id>/
  templates/
  sets/
  packages/
  installed/
  fixtures/
```

## Package Boundary

- Source workspace location does not decide whether something is a plugin; package/install/enable-disable/load lifecycle does.
- `packages/` stores `.1flowbasepkg` artifacts only.
- `installed/` stores installed package results only.
- `host-extensions/*` packages are system/root HostExtension source manifests owned by the plugin source workspace; they are not statically linked into `api-server`.
- `runtime-extensions/*` packages implement registered runtime slots through plugin-runner.
- `capability-plugins/*` packages contribute workspace-selected app/workflow capabilities.
- `sets/minimal.yaml` and `sets/default.yaml` select package sources for deployment assembly. They do not make plugin source code part of `api-server`; selected plugins still move through package/install/load lifecycle.

## Host Assembly Boundary

- `api-server` owns loader, policy, inventory, infra bootstrap, route mount, and boot assembly.
- The plugin source workspace owns HostExtension source manifests and templates.
- RuntimeExtension packages continue to use `plugin-runner`.
- CapabilityPlugin packages remain workspace-selectable abilities.
- `capability-plugins/1flowbase` is the Boot Core-owned exception: its system JSX block is visible in every workspace without assignment and cannot be deleted through plugin management.

## Data Source Plugin Rules

1. Data-source plugins must declare `consumption_kind: runtime_extension`.
2. Data-source plugins must use `slot_codes: [data_source]`.
3. Data-source plugins implement `validate_config`, `test_connection`, `discover_catalog`, `describe_resource`, `preview_read`, and `import_snapshot`.
4. Data-source plugins must not run platform migrations or write the platform database directly.
5. OAuth callback endpoints belong to the host, not the plugin.
6. Preview access is temporary; only host-controlled import writes durable platform state.

## Host Boundary

- Main repo durable storage officially supports PostgreSQL only.
- Data-source plugins integrate external databases, SaaS APIs, or HTTP systems through the runtime-extension host path.
- The host owns installation, assignment, secret storage, validation workflow, preview session lifetime, and durable imports.

## Template

- Start from `plugins/templates/data_source_http_fixture` for an external data-source runtime-extension fixture.
- Keep the method names and JSON output shapes aligned with the runtime contract.
- Replace the shell fixture runtime with your real executable after the package loads cleanly through `plugin-runner`.
