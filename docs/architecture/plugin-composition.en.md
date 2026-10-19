# Plugin composition and event delivery

[中文](plugin-composition.md)

This document describes current contracts, not a release acceptance claim. See the [historical #2007 test inventory](archive/2007/plugin-composition-test-batch.md) for that stage's commands. Later namespaced event contracts are described in the [invocation lifecycle](interface-lifecycle.md); Create → A → B/C below is a concrete example.

## Plugin activation and capability approval

User operation permission and plugin capability authority are separate decisions. Root may manage plugins without role configuration; other users need the corresponding enable, disable, or version-switch operation.

Every entry point activates or switches through `PluginManagementService`. After target identity validation, the shared preparation service reads the verified manifest, checks or prepares capability approvals, applies declared schema, and enters the existing activation/generation publication flow. Managed activation fails closed if preparation is not configured.

An explicit root enable/switch approves the version's declared, host-supported capabilities, including previously revoked capabilities. Ordinary restart and runtime rebuild do not create approvals. Non-root operators must use exact active approvals for the target installation; enable/switch cannot inherit approvals from another version, expand scopes, or restore revoked authority. The separate authorization-view operation is not required.

Approval and additive schema preparation precede activation. If later activation fails, completed approvals and schema remain and retries reuse exact records. Failure must not revoke concurrent approvals or delete data. Existing lifecycle owners retain desired-state recovery and candidate publication responsibilities. Runtime calls continue to enforce current authority, ownership, and revocation.

## Plugin selection and implementation limits

Choose the governance boundary by whether a plugin implements trusted Host internals. Then assess contribution kind, execution mode and activation scope separately. Existing `HostExtension`, `RuntimeExtension` and `CapabilityPlugin` names are not a ladder of increasing permissions.

| Dimension | Selection rule |
| --- | --- |
| Governance | Native in-process factories, authentication adapters and Host infrastructure implementations require trusted HostExtensions. Business capabilities declared through public protocols and registered/executed under Host governance belong to managed plugins. |
| Contribution | Check the contract for each provider, node, tool, data schema or settings surface. One package may contribute several kinds; declaring one does not prove support or authorization. |
| Execution | Managed subprocesses and trusted native in-process execution are distinct. A child process started by the Host is not thereby a HostExtension. |
| Activation scope | System, workspace and model requirements belong to activation/binding contracts. System-wide configuration does not itself grant access to Host internals. |

An SSH subprocess that tests connections and executes commands while declaring data, settings and operations should be designed as a managed business plugin. Physical tables, `/settings/ssh` and MCP Tools do not require reclassification as a HostExtension. Page/API access reuses existing roles and console operations; contribution grants constrain the plugin and do not replace caller authorization.

**Current entry points and boundaries:**

- Legacy workspace/model assignments still use [`PluginAssignment::new`](../../api/crates/plugin-framework/src/assignment/mod.rs). Explicit v2 `managed_service.scope: system` packages activate independently, without a business workspace assignment. Neither path grants authority to the other.
- `managed_service` declares a settings feature and method/path/JSON Schema operations; `settings_pages` references packaged TSX. The Host compiles them into one generation of console operations, routes, Canonical Interfaces, navigation, OpenAPI and MCP projections. Managed installation, upgrades and version selection validate a complete candidate before committing selection, templates and the permission catalog, then publish the generation without restarting the Host. Failed candidates leave the published generation intact. Each HTTP request selects one generation; runtime-admitted work keeps its original execution snapshot and cannot mix schemas or handlers across versions. A request still awaiting authentication or runtime admission may be rejected by current authority after a switch. Disable and grant revocation reject new execution immediately. Uninstall retires unreferenced snapshots with no durable delivery backlog before removing artifacts; live references or backlog block removal and allow retry. Business data is retained. System services use system scope and existing role operation authorization without workspace assignment.
- Successful extension actions invalidate frontend navigation, page versions, template lists, role catalogs, API documentation and MCP caches. An open template editor keeps its starting revision and unsaved draft. Server publication and frontend refresh have separate completion boundaries; closing a page or cancelling its waiter does not imply business rollback.
- An operation's optional `mcp` declaration projects fixed tools under `/plugins/{plugin_code}` through the existing MCP catalog and Interface invocation. Package declarations do not overwrite user configuration, and browser-only instances do not receive system tools. Selected instances, discovery policy and role/API authorization still govern visibility. Ordinary JSON operations also enter the interface catalog without an `mcp` field. For editable tools, create persistent `InterfaceWrapper` records with descriptions, input/output mappings and group bindings. Their IDs must not collide with fixed tools; plugin upgrades do not overwrite user configuration.
- Physical schema ownership and version-specific field projections are separate. The Host persists the exact objects declared by each version: installing a new version neither revokes the old projection nor exposes new fields to it. Membership is immutable within a version and reused on rollback. Removed fields retain physical data without granting access to the new version. Without a default-value contract, adding a required column or omitting a required column from a still-writable collection would break coexistence, so those changes are rejected. Uninstall deactivates family data access while retaining tables and version mappings; runtime callbacks still require current contribution authority.
- `process_per_call` with `stdio_json_multiplex_v1` reuses the shared SDK and Host carrier. The Host binds installation, contribution, scope and deadline to system PluginData and outbound-credential callbacks and checks live authority on every call. Credentials are encrypted at rest and omitted from ordinary page data. Plugins receive neither Host login credentials nor SQL/database connections.
- `application_logs_v1` and `http_egress_v1` are two fixed typed Host services using the SDK callback dispatcher. Contributions receive separate `application.logs.read`, `application.logs.import` (`application-logs/v1`) and `network.http.request` (`http-egress/v1`) grants. The real caller remains separate from system storage scope; each callback rechecks session, operation and contribution authority, while log use cases enforce application ACL and reception policy. HTTP uses the caller workspace Network Egress `managed_plugin` selector with the installation UUID, preserves remote status/body/redirects and releases Host leases on cancellation/deadline. Complete facts exceeding the existing 32MiB frame budget fail explicitly without truncation.
- `managed_service.worker_lifetime` defaults to `per_call`. Explicit `actor_scope` keeps the per-operation budget of `process_per_call` and the `stdio_json_multiplex_v1` wire protocol while the Host reuses the carrier. Contributions with the same installation, scope, frozen artifact and executable share a worker for one trusted actor; different actors receive separate processes. Operations must declare the same runtime entry and limits. Every call retains its own contribution permissions, credential binding and deadline. `worker_idle_timeout_ms` is a positive integer, defaulting to 1,800,000; only workers without active calls or retained resource leases may expire from idleness. Cancellation targets one correlated call. Unmount, reload and shutdown fence admitted but unpolled calls and await actual process termination and reaping before disposal completes. Short HTTP pull operations can use this contract; carrier lifetime is separate from business-session idle policy.
- Every binding sharing an actor worker may explicitly declare `runtime_worker_retention/v1`. The Host advertises support before the SDK allows `MultiplexEmitter::retain_resource().await` to acquire an RAII lease that can outlive its call. Acquisition flushes its control update; release coalesces resource-count updates, with sequence numbers rejecting stale counts. The plugin releases leases when business resources close, expire or disconnect. The Host manages worker reclamation without inferring business session deadlines. Failure, uninstall, upgrade and explicit reset override leases. Acquisition fails explicitly when an older Host does not advertise support.
- Operations default to `execution_concurrency: serialized`, holding the installation mutation lock for writes. Explicit `parallel` skips only that lock; method-based authorization and mutation semantics remain intact. After a successful `invalidates_workers: true` operation, the Host resets the mounted shared actor pool: it fences old admissions, awaits old child exits, then permits new workers. Calls admitted during the change may be cancelled; once success returns, old connections are unavailable. `invalidates_streams` independently revokes stream access.
- System scope, startup registration recovery and on-demand workers are independent dimensions. A system service remains a managed subprocess. `@1flowbase/data-table` exposes the existing table; `@1flowbase/plugin-settings` uses the current session and CSRF token. Authorization remains exclusively on the backend.

Implementation entry points are `extension-package-runtime/src/managed_service.rs`, `api-server/src/managed_services/` and the shared `runtime-extension-sdk`. This contract description does not replace centralized QA or release-artifact evidence.

Historical acceptance reports retain their original facts; they do not define current plugin selection. Evaluate old whole-package restrictions against this governance boundary and record actual code gaps separately.

## Governance and authorization

HostExtensions retain their trusted startup/restart boundary. Managed contributions execute in workers. Contribution kind, execution mode and activation scope are separate concepts; legacy manifest categories normalize input without defining whole-package permissions. Rust native libraries are not repeatedly hot-unloaded.

The current workspace-managed composition path requires installation, workspace assignment, contribution authorization, graph compilation and execution binding. A declared permission is not a grant. Installation, workspace, contribution, resource scope and exact contract version govern each call. Upgrades and old queued events do not grant authority to new candidates.

The following operations live under `installed/{installation_id}` in Extension Center. POST operations require session, CSRF and their own authorization; legacy configure permission does not replace them.

| Path | Method | Operation |
| --- | --- | --- |
| contribution-authorizations | GET | extension_center.contribution_authorizations.view |
| contribution-authorizations | POST | extension_center.contribution_authorizations.grant |
| contribution-authorizations/revoke | POST | extension_center.contribution_authorizations.revoke |
| managed-execution | GET | extension_center.managed_execution.view |
| lifecycle-deliveries/resume | POST | extension_center.lifecycle_deliveries.resume |
| managed-executions/retire | POST | extension_center.managed_executions.retire |

Grant bodies identify contribution, permission, resource scope and permission contract/version. Revocation supplies `authorization_id` and `expected_revision`. Event publishing, subscription and owned-data writes need their respective contribution grants; granting a `.data` contribution does not authorize `.events` writes.

### Managed external login authentication

`authentication_providers` declares system-scoped login types, configuration forms and public login blocks. Execution uses the `1flowbase.authentication.provider` contribution. `authentication.verify` and `network.http.request` require separate, exact contribution grants. Installation does not grant authority; the existing activation owner prepares supported grants when root enables the extension. Authentication reuses managed workers, the six runtime ports and the Host HTTP service.

The Host owns connection-bound encrypted secrets, the fixed public callback origin, state, PKCE, browser binding and one-time transaction claims. Anonymous authentication egress accepts only an active system authentication contribution and checks the current installation, version and grants without inventing a user actor. Raw codes, verifiers and cookies stay inside the trusted authentication boundary; business bindings receive verified identities. HTTP traces omit authorization and callback query parameters.

The plugin verifies the upstream identity and first-registration eligibility. The Host resolves explicit bindings by stable external subject; existing bindings bypass signup and account-age gates. Unknown identities create an ordinary member, default role and external binding atomically only when registration is enabled, eligibility passes and the verified primary email is available without conflict. No password identity is created and matching emails never merge accounts. Linking an existing account requires its current session and CSRF authorization, with the original session checked again on callback. Disabled users, entries or plugins and revoked contribution grants must prevent session issuance.

## Frozen execution and transactions

The original concrete entry is `model_definitions.create`, HTTP `POST /api/console/settings/data-models/model-definitions`. HTTP and MCP invoke the same typed Kernel. Authorization, Admission and Before can veto; After, Failure and Completion observe without replacing the primary result. Authentication stays in the trusted Host.

Invocations freeze graph, binding, artifact, generation and execution identity. Candidates must still be authorized at publication; old calls retain their snapshot and new calls use a complete new snapshot, never a mixture or a lookup of latest.

In the example, A contributes Create.before hooks and an owned collection. Its event handler consumes `model_definition.committed@v1` and publishes `acme.composition-a.processed@1`. B and C consume it into their own `processed_models` collections. Managed schema installation precedes activation; plugins receive no SQL access. The example payload contains constrained `model_id`, `status` and `result_reference`; the Host supplies trusted identity and causation.

The Create fact and Outbox targets commit in the business transaction. A's derived publication commits in a separate transaction. B/C effects and consumption receipts commit atomically through PluginData. Idempotency uses installation/workspace/contribution/event, not worker generation. This does not guarantee exactly-once external side effects. The in-process AfterCommit lane does not replace durable Outbox delivery.

The legacy Create veto is adapter-specific; generic managed Before only observes. The [lifecycle permission table](plugin-lifecycle-contracts.en.md#managed-interface-phase-permissions) owns these rules; this example does not grant broader plugin authority.

## History, recovery and retirement

Targets retain exact epoch, graph, handler/version, artifact and binding. Missing old epochs after restart pause conservatively; they never resolve to the current version. Re-granting or re-enabling does not automatically resume delivery. Resume requires event/subscriber IDs and the expected graph/handler/version, then revalidates exact identity and current authority. Retirement rejects current targets, live references and pending durable work. Unknown legacy history remains conservative and blocks unsafe recovery/deletion. Each claim has a fresh identity; a stale ACK cannot complete a newer claim.

GET `managed-execution` preserves `deliveries` and `deliveries_truncated`, adding `next_cursor`. Pass that cursor unchanged as the next request's `cursor`; omission starts the first page, and a null cursor marks the end. Cursors bind installation and workspace and confer no authority. Malformed or mismatched cursors are rejected. Resume/retire responses still return the first page.

Metadata keyset pagination orders by `(event_id, subscriber_id)` and defaults to 256 candidate rows per page, not a total history limit. Continuation uses the last scanned candidate, even if ownership filtering leaves an empty page. Continue whenever `next_cursor` is present. Static history is traversable without duplication or omission; separate requests do not share a database snapshot. Exact resume and backlog safety checks remain independent of display pagination and only load the selected payload when necessary.

## Resource ownership and shutdown

Current workspaces, retained snapshots, frozen references and retirement markers are not rejected at arbitrary fixed counts. Reference counts and notifications still enforce closing and retirement. Retained snapshots are released only after explicit retirement checks, including durable backlog. Retirement markers persist until Host shutdown because another graph can share the same worker generation and otherwise recreate the retired target. They are safety state, not disposable cache. This does not promise constant memory or infer deployment capacity from a development machine.

Other existing lanes retain distinct controls: store/composition owners admit 128 operations; managed mounts allow 4096, calls 32 per scope and 128 globally; hashing/loading admits 4; managed request/stdout/stderr frames are 1 MiB; the temporary AfterCommit lane holds 4096 entries. These controls do not define the capacity of historical records or workspaces. The Outbox dispatcher uses batches of 32, a 10-second handler deadline within a 30-second lease, five attempts and 4096-byte diagnostics.

Backlog safety checks scope installation, historical authority, contributions and unfinished status in SQL. Their existing 4096-row/5-second inspection budget returns `managed_backlog_check_busy` (HTTP 409) if emptiness cannot be proved; completed history is excluded. Native targets use SQL EXISTS. Unknown or unverifiable legacy scope blocks cleanup rather than being bypassed through pagination.

Shutdown closes dispatcher admission and waits for its batch, then composition operations, store Create/E2 operations and frozen references (15 seconds each), followed by Host hash and scope/worker cleanup (5 seconds each). Already admitted Create work may still freeze its publication. Timeout reports unfinished work and retains database backlog. Cancellation retains the child lease until kill/reap; failed reap retains the permit. Required event channels preserve backpressure; diagnostics preserve drop counts. Capacity failure is execution failure, not permanent contract invalidity.

## Immutable reinstall

The original archive SHA-256 identifies an installed `plugin_id + version`. Reinstalling exactly those bytes may restore missing artifacts while retaining installation ID, disabled intent, contribution metadata, authority revisions and backlog. Repacking different bytes is rejected even if the manifest looks identical. Admission and commit share a database identity lock, including single-connection pools and concurrent first installs.

Legacy installations without a reliable checksum cannot adopt a new archive identity. A new version is a separate installation and explicit candidate switch; neither grants nor frozen deliveries transfer automatically. Historical ownership comes from durable installation/workspace/target identity, not today's manifest. Missing old graphs prevent resume and protect needed artifacts from deletion.

The provider declares the public entry click behavior with `entry_action`: `render_ui` (default) displays its UI block; `external_login` starts Host-owned external authentication on an explicit click. The public DTO projects the active provider definition, rather than inferring behavior from labels, UI source or frontend auth-type branches. Mounting, single-entry discovery, direct links and failure returns never start authorization automatically.
