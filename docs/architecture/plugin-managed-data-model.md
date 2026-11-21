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

## Provider exports and consumer prerequisites

System managed-service plugins share resources through verified provider declarations and
explicit consumer prerequisites, not a deployment participant registry. A provider keeps
its own `data_models`, `record_resources` and credentials and can install, upgrade and
enable independently. It exposes only selected resources in `resource_exports`:

```yaml
resource_exports:
  - resource_code: hosts-v1
    collections:
      - collection_code: hosts
        access: read_only
    record_resources:
      - resource_code: hosts
        collection_code: hosts
        id_field: host_id
    credentials: read_only
```

An export cannot invent ownership or omit the record protection for an exported
collection. `read_only` and `read_write` bound collection and credential operations
separately; omitted credentials remain private. An export is not itself a grant to
another plugin. No publisher-specific Host exception or environment allowlist is needed.

A consumer declares its prerequisite and the resource subset it requests:

```yaml
plugin_dependencies:
  - publisher_namespace: example
    plugin_code: ssh
resource_imports:
  - provider: {publisher_namespace: example, plugin_code: ssh}
    resource_code: hosts-v1
    collections:
      - collection_code: hosts
        access: read_only
    record_resources: [hosts]
    credentials: read_only
```

The consumer does not repeat the provider's table schema in `data_models` or create
another physical table. It may declare local operation/action mappings for imported
record resources; collection ownership, identity field/type and physical schema resolve
from the verified provider. Requested rights cannot exceed the provider's export.
A callback cannot choose a different publisher, provider, scope or physical owner.

### Approval and live authority

Upload/install alone grants no cross-plugin access. Explicit Root enablement uses the
existing contribution-approval path to materialize the consumer's exact imported
collection and credential capabilities. Nonroot enablement requires the existing exact
active grants and must not create missing grants or restore revocations. Imports do not
replace the consumer's authority subject with the provider's permissions.

Every data/credential callback verifies the live consumer installation, contribution,
grants, storage/secrets permission, scope and deadline while holding its authority lease.
Provider discovery uses the same lease transaction's installation inventory. It then
checks the currently enabled provider and its export before resolving physical access.
Both export/import access bounds and the consumer's current grants must permit the
operation. A disabled or revoked consumer cannot retain access through a cached binding.

Authority observations for resource callbacks, execution-gate checks and record admission
use compatible shared leases. The lease remains held through the actual resource
operation; grant/revoke, provider selection and lifecycle writers retain the same exclusive
fence and wait for every reader. Authority reads do not become authority writes merely
because the separately authorized data operation writes a provider collection. This lets
concurrent live sources and per-frame record checks overlap without weakening revocation.

Record admission, capability queries, policy editing and SQL predicates use the resolved
provider identity and protected resource, but retain the real caller's workspace. Operation
permissions remain those of the consuming plugin. A frozen registration is rejected if
its live provider resolution no longer matches; it cannot keep stale provider authority.

### Ownership, upgrade and lifecycle

Provider-owned schema reconciliation retains the ordinary stable
`publisher_namespace/plugin_code` owner and physical names. Imported callbacks use the
provider's actual schema-version projection; enabling a consumer never creates a
synthetic shared schema version or adds objects to another version's immutable membership.
Existing field/type/nullability drift checks still apply. Credential lookup and encryption
AAD retain the exact original publisher/plugin/scope/credential tuple. For SSH this remains
`taichuy/ssh`, including existing target and saved jump-host credentials; no copying,
reencryption or table recreation is required by resource sharing.

Consumer enablement fails with a structured dependency error when the prerequisite is
missing, disabled, ambiguous, incompatible or lacks the requested export. Errors identify
the provider/resource, rather than becoming an internal 500. An enabled consumer blocks
provider disable/uninstall and incompatible provider replacement; the error identifies
the consumers that must be disabled first. Disabling the consumer releases that protection.
The authoritative state transition rechecks dependencies in its transaction using the
system managed-workspace lock, coordinated with callback authority leases, not only an
unlocked service preflight. Approval and managed-service generation publication take
effect through the normal enable path without restarting the API.
