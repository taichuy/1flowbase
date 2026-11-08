# Console Operation Registry Hygiene

This gate keeps the compiled `/api/console` operation contract observable for CI.

Run it directly with:

```bash
node scripts/node/console-operation-registry-hygiene/cli.js
```

The authoritative checks run serially:

- `cargo test -p api-server --test api_private_routes migrated_assembly_contains_every_console_router_owner_assembly`
- `cargo test -p api-server --test api_private_routes console_route_assembly`

With no arguments, the gate runs the Rust `console_operation_inventory` exporter. The exporter
uses `cargo run --profile test` to match the preceding assembly checks while building a normal
binary from the current candidate; it does not enable test-only features. Matching dependencies
can be reused by Cargo, but a different feature graph or build fingerprint may still require
compilation. It uses the same Core boot registry and route assembly as the API server and writes the current
`1flowbase.console-operation-inventory/v1` snapshot under `tmp/test-governance/`. The snapshot
also carries its compiled route assembly and `zh_Hans` / `en_US` locale evidence. The gate then
compares it with `compiled-inventory-baseline.json`; missing current or baseline snapshots fail
closed.

`--compiled-inventory <path>`, `--baseline-inventory <path>`, and `--locale-dir <path>` remain
available for fixture and drift tests. These inputs never cause the Node gate to reconstruct Axum
routes from Rust source.

Reports are always written to:

- `tmp/test-governance/console-operation-registry-hygiene.json`
- `tmp/test-governance/console-operation-registry-hygiene.md`

Source scans are advisory warnings only. Compiled test failures and compiled inventory,
locale, migration, and diff violations are errors.
