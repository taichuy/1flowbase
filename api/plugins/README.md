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
- `runtime-extensions/*` packages implement registered runtime slots through the in-process runtime-extension host.
- `capability-plugins/*` packages contribute workspace-selected app/workflow capabilities.
- `sets/minimal.yaml` and `sets/default.yaml` select package sources for deployment assembly. They do not make plugin source code part of `api-server`; selected plugins still move through package/install/load lifecycle.

## Host Assembly Boundary

- `api-server` owns loader, policy, inventory, infra bootstrap, route mount, and boot assembly.
- The plugin source workspace owns HostExtension source manifests and templates.
- RuntimeExtension packages are reconciled and executed by the single Host owned by `api-server`.
- CapabilityPlugin packages remain workspace-selectable abilities.

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

## CapabilityPlugin Credit Commands

Only a `verified_official`, `process_per_call` CapabilityPlugin may declare narrow
`permissions.credit` values (`credit.grant`, `credit.charge`, `credit.adjust`,
`credit.toggle`, `credit.refund`, `credit.reserve`, `credit.settle`, or
`credit.release`). The plugin never receives database access.

An executing node requests one Core command by returning an object containing
`_1flowbase_credit_command`. The command supplies `user_id`, decimal `amount`,
`credit_unit: USD`, `reason`, paired `source_type/source_id`, and an
`idempotency_key`. Reserve also supplies `provider_invocation_id` and
`pricing_rule_id`; settle/release supply `billing_session_id`. The host fixes the
workspace and plugin actor from the trusted invocation context, checks the
manifest permission, executes the account transaction, removes the command, and
adds `_1flowbase_credit_result` to the node output. Replaying the same key with a
different payload is rejected.

Successful commands publish `Credit*` facts through the durable outbox. Event
delivery is at-least-once, so consumers must deduplicate by `event_id`.

## Template

- Start from `plugins/templates/data_source_http_fixture` for an external data-source runtime-extension fixture.
- Keep the method names and JSON output shapes aligned with the runtime contract.
- Replace the shell fixture runtime with your real executable after the package passes runtime-extension-host conformance tests.
