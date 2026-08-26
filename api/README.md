# API Workspace

## Module Map

- `apps/api-server`: Axum HTTP entrypoint for public, console, and runtime routes
- `crates/control-plane`: backend application services and permission-checked state transitions
- `crates/runtime-core`: runtime resource descriptors, registries, and capability slot engine
- `crates/plugin-framework`: Plugin contracts, including model-provider, capability, and data-source runtime packages
- `crates/storage/durable/core`: stable `storage-durable` backend boundary
- `crates/storage/durable/postgres`: `storage-durable-postgres` implementations, runtime builders, and migrations
- `crates/storage/ephemeral`: non-durable session and ephemeral coordination adapters
- `plugins/host-extensions`, `plugins/runtime-extensions`, `plugins/capability-plugins`: HostExtension, RuntimeExtension, and CapabilityPlugin source workspaces
- `plugins/packages`, `plugins/installed`: packaged `.1flowbasepkg` artifacts and installed package results
- `plugins/templates/data_source_http_fixture`: external data-source runtime-extension template, not the only plugin template

## Plugin Layering

- `apps/api-server` owns loader, policy, inventory, infra bootstrap, route mount, and boot assembly.
- `plugins/host-extensions` owns HostExtension source manifests; `plugins/templates` owns source templates.
- RuntimeExtension packages run through the in-process `crates/runtime-extension-host` owned by the single `api-server` composition root.
- CapabilityPlugin packages are workspace-selectable abilities.

## Verification

Run from the repository root:

```bash
node scripts/node/verify-backend.js
```

The verification script caps `cargo` build and test concurrency at half of the machine's available CPU so full backend runs stay within a safer resource envelope by default.
