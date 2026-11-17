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

## Schema Application Safety

Schema application uses the ownership ledger, registered business-table inventory,
structural drift checks, a transaction and a lock timeout. Table byte size is not a
business capacity contract: the host does not reject additive schema changes using
a fixed table-size threshold. PostgreSQL DDL failures roll back both physical
changes and ownership publication; retained version membership remains intact.

## Deployment-approved shared resources

A system managed-service manifest can reference `shared_resource_contract: example/hosts/v1`.
The reference grants no authority by itself. The API composition root loads the immutable
`PLUGIN_SHARED_RESOURCE_CONTRACTS_JSON` deployment registry; an unknown reference or a
publisher/plugin identity outside its participants fails activation and callbacks.
An empty registry preserves ordinary plugin isolation. Packages cannot register contracts.

A registry entry has this shape (names are examples, not built-in exceptions):

```json
[{
  "contract_id": "example/hosts/v1",
  "owner": {"publisher_namespace": "example", "plugin_code": "ssh"},
  "schema_version": "shared-v1",
  "participants": [
    {"publisher_namespace": "example", "plugin_code": "ssh"},
    {"publisher_namespace": "monitor-publisher", "plugin_code": "monitor"}
  ],
  "collections": ["hosts", "credential_staging"],
  "record_resources": ["hosts"],
  "record_resource_bindings": {"hosts": {"collection_code": "hosts", "id_field": "id"}},
  "credentials": true
}]
```

Every participant declares the complete shared collection schema in `data_models` and
the exact protected resource-to-collection/identity bindings. A package cannot omit or
retarget a protected resource to avoid its row policy.
Activation reconciles it under the configured original owner and fixed schema version,
including when no other participant is installed. Existing same-version object membership
is immutable; incompatible types/nullability or partial declarations fail through the
normal schema ownership adapter. Historical plugin-version projections remain intact.

Data and credential callbacks first validate the original installation, contribution,
live grants, storage/secrets permission, scope and deadline while holding its authority
lease. Only then does the host resolve the configured physical owner. The worker cannot
choose an owner. Credential lookup and encryption AAD continue to use the original
publisher/plugin/scope/credential tuple, so existing encrypted credentials remain usable.
A participant's disable/revoke fences its own callbacks; another enabled participant
retains its own authority and the stable shared schema projection.

Record admission, capability queries, policy editing and SQL row predicates use the
same effective original owner, resource code and caller workspace. Policy changes revoke
newly denied streams across active participants of that identity. Operation permissions
continue to belong to the calling plugin. Registry changes require process restart;
removing a participant withdraws sharing without mutating persisted data or credentials.
