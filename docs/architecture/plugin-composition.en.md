# Plugin composition and event delivery

[中文](plugin-composition.md)

This document describes current contracts, not a release acceptance claim. See the [historical #2007 test inventory](archive/2007/plugin-composition-test-batch.md) for that stage's commands. Later namespaced event contracts are described in the [invocation lifecycle](interface-lifecycle.md); Create → A → B/C below is a concrete example.

## Governance and authorization

HostExtensions retain their trusted startup/restart boundary. Managed contributions execute in workers. Contribution kind, execution mode and activation scope are separate concepts; legacy manifest categories normalize input without defining whole-package permissions. Rust native libraries are not repeatedly hot-unloaded.

Activation requires installation, workspace assignment, contribution authorization, graph compilation and execution binding. A declared permission is not a grant. Installation, workspace, contribution, resource scope and exact contract version govern each call. Upgrades and old queued events do not grant authority to new candidates.

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

## Frozen execution and transactions

The original concrete entry is `model_definitions.create`, HTTP `POST /api/console/settings/data-models/model-definitions`. HTTP and MCP invoke the same typed Kernel. Authorization, Admission and Before can veto; After, Failure and Completion observe without replacing the primary result. Authentication stays in the trusted Host.

Invocations freeze graph, binding, artifact, generation and execution identity. Candidates must still be authorized at publication; old calls retain their snapshot and new calls use a complete new snapshot, never a mixture or a lookup of latest.

In the example, A contributes Create.before hooks and an owned collection. Its event handler consumes `model_definition.committed@v1` and publishes `acme.composition-a.processed@1`. B and C consume it into their own `processed_models` collections. Managed schema installation precedes activation; plugins receive no SQL access. The example payload contains constrained `model_id`, `status` and `result_reference`; the Host supplies trusted identity and causation.

The Create fact and Outbox targets commit in the business transaction. A's derived publication commits in a separate transaction. B/C effects and consumption receipts commit atomically through PluginData. Idempotency uses installation/workspace/contribution/event, not worker generation. This does not guarantee exactly-once external side effects. The in-process AfterCommit lane does not replace durable Outbox delivery.

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
