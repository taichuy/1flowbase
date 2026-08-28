# Plugin Managed Data Model v1

The plugin manifest is the only desired-state declaration for managed data. The Host compiles that declaration with the registered business-table catalog and the durable ownership ledger, then the PostgreSQL adapter previews and applies an additive plan.

```text
manifest data_models
  + registered business tables
  + plugin_schema_ownership
  -> EffectiveManagedSchemaPlan
  -> preview
  -> reconcile receipt
```

## Stable boundaries

- `extension-contracts` owns the manifest contribution types.
- `plugin-framework` owns deterministic namespacing, additive policy, drift checks, and lifecycle plan compilation.
- `control-plane-contracts` owns the storage-facing managed-schema Port.
- `storage-durable-postgres` alone performs catalog inspection, advisory locking, DDL, ownership persistence, and backup/recovery inventory checks.
- `api-server` binds install/update and disable/uninstall lifecycle events to the compiled plan.

Runtime extensions never receive SQL, a database connection, a local path, or a general query interface. A target table is eligible when it is present in the existing model-definition registry; it does not declare a second extension slot or per-plugin allowlist.

## v1 lifecycle

Install and update inspect, compile, preview, reconcile, and return a typed receipt. Disable and uninstall retain physical tables and columns while marking ownership inactive. Re-enabling or upgrading reconciles the same stable physical identities. Drop, rename, type changes, nullability changes, defaults, constraints, triggers, functions, and runtime DDL are outside v1.

The full PostgreSQL logical backup contains both ownership rows and physical objects. Backup preflight and staged recovery verify that every active or retained ownership record still resolves to its table or column before the database can be promoted.
