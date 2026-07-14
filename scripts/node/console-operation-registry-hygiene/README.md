# Console Operation Registry Hygiene

This gate keeps the compiled `/api/console` operation contract observable for CI.

Run it directly with:

```bash
node scripts/node/console-operation-registry-hygiene/cli.js
```

The authoritative checks run serially:

- `cargo test -p api-server migrated_assembly_contains_every_console_router_owner_assembly`
- `cargo test -p api-server console_route_assembly`

An optional `--compiled-inventory <path>` supplies a serialized
`1flowbase.console-operation-inventory/v1` inventory plus a `route_assembly` sidecar. A
`--baseline-inventory <path>` enables deterministic missing and permission-expansion diff
reporting. `--locale-dir <path>` must contain `zh_Hans.json` and `en_US.json` when an inventory
is supplied.

Reports are always written to:

- `tmp/test-governance/console-operation-registry-hygiene.json`
- `tmp/test-governance/console-operation-registry-hygiene.md`

Source scans are advisory warnings only. Compiled test failures and compiled inventory,
locale, migration, and diff violations are errors.
