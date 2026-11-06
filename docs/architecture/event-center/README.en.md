# Event center

[中文](README.md)

The Event Center lives at `/settings/events`. Custom definitions, workflow trigger configuration, and communication command bindings are durable. Custom event instances, Event Center delivery queues, and observations are process-local and may be lost on restart. Existing runtime, lifecycle, and communication sources keep their original persistence, retry, fencing, and acknowledgment semantics.

Each tab has its own URL: `/settings/events?tab=catalog`, `/settings/events?tab=queues`, and `/settings/events?tab=activity`. Opening `/settings/events` selects the catalog. Reloading and browser back/forward preserve the tab identified by the URL. The page uses the same shared table toolbar and column settings as Application management, without a separate title panel.

## System sources and original queues

The catalog distinguishes declarations from attached sources. `source.selection` identifies the stable source and explicit scope; `point_id` identifies an extension point, while `contract_id` and `contract_version` identify the business contract. Schemas come from typed owners or managed declarations. The shared JSON viewer displays objects, booleans, and `null` unchanged. Runtime streams advertise the actual `RuntimeEventEnvelope` schema rather than inventing a schema for each dynamic `event_type`.

Attached families include runtime required/diagnostic/after-commit streams, communication commands, native model and credit notifications, and managed publications permitted by the frozen graph. Private Notify/watch/socket primitives are outside this event inventory. Binding, coverage, and supported phases reflect actual adapters; declarations and unbound sources cannot trigger workflows.

Current queues separates original source queues from Event Center workflow queues. Runtime counters come from the original channels; durable pending/paused/claimed counts come from read-only aggregation of the existing outbox. Unknown cumulative counters remain unknown. Reading observations never dequeues a message. Consumption is original ownership handoff before a handler result, retry decision, or acknowledgment. Required delivery and diagnostic drop behavior remain unchanged.

Each scope retains its own activity, memory accounting, and loss window. Workspace and System remain distinct even when their UUID values coincide. Ordinary users see their current workspace; root can also inspect System and Unbound. Historical outbox rows without trusted scope remain Unbound. Additional durable metadata contains scope, producer identity, and workflow ancestry only, never another canonical payload. Queue refresh errors are explicit rather than represented as empty queues.

Workflow configuration persists an optional source selection; omission preserves custom-event behavior. Only root may select System sources, and Workspace sources cannot cross workspaces. Trusted workflow ancestor IDs propagate through execution, durable publication, and internal MCP calls. A workflow cannot reenter the same causal chain; there is no arbitrary depth cap. Observations remain payload-free; source bodies are projected lazily only for actual matching targets. Run records continue to store mapped inputs and necessary identity metadata only.

## Trigger phases

| Phase | Meaning |
| --- | --- |
| `subscribed` | A new effective logical subscription is activated. Restoring configuration, unchanged saves and additional workers for the same subscription do not create new subscription facts. |
| `published` | A valid event is published and admission is attempted independently for each current business consumer. |
| `consumed` | A business delivery is dequeued and ownership is transferred to its consumer, regardless of its eventual business outcome. |

Independent subscriptions have separate queues. Workers within one subscription compete for deliveries. Lifecycle observers receive control signals; receiving one never recursively emits another consumption fact for the original event. A consumption observer alone does not manufacture a business consumption.

An `EventDelivery` continues to hold its resource charge until its guard is released. Dequeue counts, retained resources and successful business execution are separate concepts. Inspection reads snapshots without consuming messages.

## Configuration flow

1. Create a `custom.<name>` contract with a version and payload schema. System and plugin declarations are read-only; only implemented source phases can be selected.
2. Create a Workflow application with the Event trigger. Configure its event, phase and input mapping, then publish it. Missing publication, authority or source availability prevents activation and is reported on the current queues page.
3. Map input names to RFC6901 pointers such as `/payload/order_id`. Existing Start string/number/boolean rules apply. Only explicitly mapped inputs and necessary event identities are included in workflow run records, not an extra copy of the entire event context.
4. Communication bindings support `user/<name>` commands. Map `/arguments`, `/input_id`, `/conversation_id` and `/instance_id` into event fields. Names must not conflict with builtin, plugin or other user commands, including `/new`, `/model` and `/help`.
5. Use existing workflow notification actions to send messages. A command acceptance reply does not confirm workflow completion or notification delivery.

Publication reports admission per subscription. Zero targets is valid but does not retain events for future subscribers. Partial rejection does not roll back other targets or automatically retry all targets. A consumption fact and admission of its observer workflow are separate outcomes. An uncertain prior command attempt is not blindly published again.

## Host resource policy

`API_EVENT_CENTER_POLICY` accepts an explicit JSON host policy. There is no guessed production capacity default. Without a policy, configuration remains available but the runtime is unavailable; the page reports that condition. Restart the host after changing the policy.

| Field | Budget |
| --- | --- |
| `bus_limits.max_items` | Reserved, buffered and in-flight deliveries combined |
| `bus_limits.max_json_bytes` | Combined upper bound of serialized JSON bytes, not process RSS |
| `bus_limits.max_waiters` | Waiting operations |
| `bus_limits.max_topics` / `max_consumers` | Queue generations and consumer owners |
| `bus_limits.max_topic_name_bytes` | Bytes retained by queue directory names |
| `observation_max_bytes` | Shared charge for recent activity and workspace accounting metadata |
| `observation_ttl_ms` | Observation window in milliseconds |

Set values for the deployment's resources. Small fixture budgets exercise boundary conditions and are not product limits. Snapshots identify the host instance; a local value is not a multi-host total. Observations retain metadata without copying event payloads. `observation_bytes` reports this workspace's currently visible activity charge (serialized JSON plus entry metadata) and available accounting ledger charge. The shared host budget also bounds accounting metadata; these charges are not process RSS.

`observation_loss` exposes workspace-only `expired`, `evicted` and `oversize_dropped` counts. `expired` means an activity reached its TTL; `evicted` means it was removed to make room before TTL; `oversize_dropped` means an incoming activity could not fit alongside its own accounting ledger. These count observation records, not unique events or business delivery losses. Counts saturate at the unsigned 64-bit maximum.

Accounting covers a finite window starting at `window_started_at_unix_ms`, with `window_duration_ms` equal to twice the configured activity TTL (saturating at the unsigned 64-bit millisecond maximum). The extra TTL lets an otherwise quiet workspace see expiration at the exact activity TTL boundary. Snapshot reads project expiration without changing storage or consuming deliveries; a subsequent write materializes the same expiration once. Activity may originate before the current accounting window; its loss is counted if the loss occurs within that window.

At the accounting window's end, coverage becomes unavailable until the next record starts a new window. Budget pressure can also evict a workspace's accounting ledger. In either case `accounting_available` is false, the start is null, and zero counters mean missing coverage, not proof of no loss. A new window never inherits prior counts. A zero TTL or a budget smaller than one ledger cannot offer accounting coverage. Stale storage is reclaimed on the next write; retained storage remains bounded even without writes. Ledgers are never an unbounded lifetime workspace map, and another workspace's losses are never added to the caller's counters. Restart can lose both activity and counters.

## Ownership and authorization

The backend owns the `system.events` Settings registration and authorizes each HTTP method/route independently. Services and repositories enforce workspace isolation. Catalog access does not grant create, update or runtime-inspection access.

The service reuses the host EventBus; workflows reuse the existing publication and execution engine; communication plugins retain provider-specific Receive/Send duties. Existing durable Outbox and diagnostic stream contracts are unchanged.

Use `node scripts/node/tooling.js settings-registry register <descriptor.json>` to edit the canonical Core registration and verify the compiled inventory during acceptance. Descriptors contain `feature_id`, `console_surface` and `api_routes`; the tool does not introduce another runtime permission map.

The only new tables store configuration: `event_definitions` belongs to the `system.events` selective-backup feature and `workflow_event_triggers` to `system.applications`. Existing restore dependency checks apply. Selecting application configuration does not silently copy event definitions owned by another feature. Builtin metadata protects structure and generic record writes; dedicated services own configuration changes.

## Verification

Implementation evidence comes from event-service unit tests, the `event_center_configuration` PostgreSQL target, event routes in `api_system_settings`, event workflow/communication cases in `api_private_contracts`, frontend feature tests and page-debug evidence. Finite design models do not replace implementation tests.

This mechanism does not promise replay, restart recovery, cross-host broadcast or exactly-once external effects. Those capabilities require a separate scope decision; observation logs must not silently become a recovery store.
