# Event center

[中文](README.md)

The Event Center lives at `/settings/events`. Custom definitions, workflow trigger configuration, and communication command bindings are durable. Custom event instances, Event Center delivery queues, and observations are process-local and may be lost on restart. Existing runtime, lifecycle, and communication sources keep their original persistence, retry, fencing, and acknowledgment semantics.

Each tab has its own URL: `/settings/events?tab=catalog`, `/settings/events?tab=queues`, and `/settings/events?tab=activity`. Opening `/settings/events` selects the catalog. Reloading and browser back/forward preserve the tab identified by the URL. The page uses the same shared table toolbar and column settings as Application management, without a separate title panel.

## System sources and original queues

The catalog manages current event definitions. Within a scope, `contract_id` identifies an event and its current schema; Event Center does not use contract versions. Creation, subscriptions, communication event bindings and Event Center signals do not carry a version. `source.selection` identifies the stable source and explicit scope; `point_id` in technical details identifies its underlying extension point. Schemas come from typed owners or managed declarations. The shared JSON viewer displays objects, booleans, and `null` unchanged. Runtime streams advertise the actual `RuntimeEventEnvelope` schema rather than inventing a schema for each dynamic `event_type`.

Attached families include runtime required/diagnostic/after-commit streams, communication commands, native model and credit notifications, and managed publications permitted by the frozen graph. Private Notify/watch/socket primitives are outside this event inventory. Available trigger phases reflect actual adapters. Declaration-only templates and historical Unbound sources do not appear as duplicate ordinary catalog entries; original historical queues remain inspectable in runtime views. Catalog identity is scope plus event identifier, independent of individual run registrations.

Current queues separates original source queues from Event Center workflow queues. Runtime counters come from the original channels; durable pending/paused/claimed counts come from read-only aggregation of the existing outbox. Unknown cumulative counters remain unknown. Reading observations never dequeues a message. Consumption is original ownership handoff before a handler result, retry decision, or acknowledgment. Required delivery and diagnostic drop behavior remain unchanged.

Each scope retains its own activity, memory accounting, and loss window. Workspace and System remain distinct even when their UUID values coincide. Ordinary users see their current workspace; root can also inspect System and Unbound. Historical outbox rows without trusted scope remain Unbound. Additional durable metadata contains scope, producer identity, and workflow ancestry only, never another canonical payload. Queue refresh errors are explicit rather than represented as empty queues.

Source definitions, schemas, queue identities and activity share `observation_max_bytes`. The center owns the only queue-observation ledger. Runtime accepted/dequeued/dropped counters update one admitted queue in place; they do not clone or serialize a second queue map. Admission reserves the maximum numeric width and fixed map/handle metadata charge, rather than measuring process RSS. Queue retirement immediately releases its identity strings; surviving original owner clones retain their guard charge until final drop.

`queue_observation_complete=false` means some queue metadata was refused. This warning applies even if the visible queue list is empty; the list is partial, and absent counters are not zeros. Trigger capability and original delivery remain unchanged. The flag stays conservative until an authoritative full queue refresh succeeds or the source owner is removed. Existing durable owners retain their full absolute snapshot refresh.

Workflow configuration persists an optional source selection; omission preserves custom-event behavior. Only root may select System sources, and Workspace sources cannot cross workspaces. Trusted workflow ancestor IDs propagate through execution, durable publication, and internal MCP calls. A workflow cannot reenter the same causal chain; there is no arbitrary depth cap. Observations remain payload-free; source bodies are projected lazily only for actual matching targets. Run records continue to store mapped inputs and necessary identity metadata only.

## Catalog and definition ownership

The default columns are Name, Event ID, Source category, Scope, Enabled, Supported trigger phases, and Actions. Owning module and extension point belong in read-only details. Contract version, registration status and observation coverage are not management columns. System/plugin ownership follows the defining module. Custom events have no owning module; their workspace is represented by scope. Source category does not depend on whether a consumer executes through a plugin.

The catalog DTO exposes a boolean enabled value for user rules and null for system/plugin events. Both use switches: system/plugin switches are checked and disabled, without writing that display state back to enabled. Custom-event write endpoints cannot toggle system/plugin sources. Request failures and incomplete queue observations remain explicit rather than being hidden with the removed columns. Custom payloads are validated against the current schema. Only configuration is durable; this design does not add payload archives or a legacy workflow migration mechanism.

Existing plugin, native lifecycle and outbox protocol versions remain with their original adapters and dispatchers. They are not part of Event Center definitions, source IDs or subscription matching. There is no hidden default event version or separate version-translation registry.

## Trigger phases

| Phase | Meaning |
| --- | --- |
| `subscribed` | A new effective logical subscription is activated. Restoring configuration, unchanged saves and additional workers for the same subscription do not create new subscription facts. |
| `published` | A valid event is published and admission is attempted independently for each current business consumer. |
| `consumed` | A business delivery is dequeued and ownership is transferred to its consumer, regardless of its eventual business outcome. |

Independent subscriptions have separate queues. Workers within one subscription compete for deliveries. Lifecycle observers receive control signals; receiving one never recursively emits another consumption fact for the original event. A consumption observer alone does not manufacture a business consumption.

An `EventDelivery` continues to hold its resource charge until its guard is released. Dequeue counts, retained resources and successful business execution are separate concepts. Inspection reads snapshots without consuming messages.

## Business events and interface lifecycle

The backend declares `group_path`, source identity, scope, subtype `filters`, and schemas. GUI, Workflow, and MCP consume the same catalog. A subtype selects the original fact; it does not publish a second event.

- **Interface rules:** `before` runs after authentication, authorization, and admission, before business execution, with explicitly exposed actual `input`. `completion` runs at the logical terminal (`completed`, `failed`, `rejected`, or `cancelled`); empty `results` selects all. Streams wait for logical completion. A closed connection does not imply business cancellation. Only successful calls carry actual declared `output`; other results never fabricate successful output. Authentication rejection without a trusted workspace cannot dispatch workspace rules. Observation cannot modify input, veto execution, or replace the primary result. Undeclared fields, credentials, native handles, and raw protocol envelopes are excluded. Notification `input_payload` is a generic business JSON object, not a provider-specific schema inferred from examples.
- **Application runs:** `host.application.run.started.v1` and `host.application.run.ended.v1` project the existing run owner. The first running transition starts a run; a winning final transaction/CAS ends it as succeeded, incomplete, failed, or cancelled. `run_mode` distinguishes API, workflow HTTP, schedule, event, and debug origins. LLM callbacks, waits, and resumes do not start or end another run. Observation does not introduce a second state machine.
- **Agent Logs:** JSON/OTLP ingestion and console import expose typed business inputs and actual receipts through `host.application.operation.<interface_id>.<before|completion>`. Trusted application/workspace identities associate the operation with its application, without fabricating a FlowRun or requiring task_end. Ingestion/import completion is independent of external agent task completion. Failure does not turn a partial receipt into successful output; existing errors retain committed counts.
- **Credits:** `credit.integration_notification` has nine selectable leaves: CreditReserved, CreditSettled, CreditReleased, CreditGranted, CreditCharged, CreditRefunded, CreditAdjusted, CreditChargeSettingChanged, and CreditCommandRejected. `/payload/event_type` selects one existing fact and preserves its publication/consumption identity.
- **Communications:** `system.communication.received` follows deduplicated message commit; duplicates do not emit another fact. Text/File content includes actual MIME and file metadata, without download credentials. `system.communication.send_result` offers accepted, failed, and unknown leaves, retaining reply linkage to the original provider message. Accepted means provider acceptance, not delivery or read. Interrupted recovery can report unknown and must not cause blind retries. WeChat currently sends text only; incoming files distinguish images, audio, and video through MIME.

```text
Interface -> before(input) -> logical terminal -> completion(output or terminal/classification)
Business owner -> commit/actual receipt -> event -> independent subscription queues -> competing consumers
Application run -> first running -> waits/callbacks/resumes -> committed terminal -> ended
```

Interface completion, application termination, subscription activation, queue handoff, and notification delivery are independent boundaries. Cross-subscription broadcast is fanout, not an additional lifecycle state.

Legacy free-schema records remain readable and retain their existing publication contracts. Records without `interface_event` are read-only in the editor; the old API cannot create new free-schema events. Legacy communication commands cannot publish fabricated interface-rule facts. The additive migration adds nullable rule metadata without deleting configuration in other environments. The development inventory found no legacy definitions, workflow triggers, or communication event-command references; that finding does not describe every deployment.

## Configuration flow

1. Create an interface event rule: select a declared Canonical Interface operation and Before or Completion. Completion can select all terminal results or a subset. Give the rule a name and inspect its inherited, read-only schema. The backend generates its event identifier; users no longer author arbitrary `custom.*` identifiers or schemas. Authorized users can toggle their rules. System and plugin events use checked, disabled switches.
2. Create a Workflow application with the Event trigger. Configure its event, phase and input mapping, then publish it. Missing publication, authority or source availability prevents activation and is reported on the current queues page.
3. Map input names to RFC6901 pointers such as `/payload/order_id`. Existing Start string/number/boolean rules apply. Only explicitly mapped inputs and necessary event identities are included in workflow run records, not an extra copy of the entire event context.
4. Legacy free-schema communication bindings support `user/<name>` commands. Map `/arguments`, `/input_id`, `/conversation_id` and `/instance_id` into event fields. Names must not conflict with builtin, plugin or other user commands, including `/new`, `/model` and `/help`.
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

The `event-business` scope in `event-lifecycle.yml` runs the focused business-event Test Batch and retains the exact source SHA, nonzero test results, and matching host candidate. Interface observation, application terminals, credit filters, communication deduplication/results, and permissions have separate evidence; source checks do not replace runtime verification.

Implementation evidence comes from event-service unit tests, the `event_center_configuration` PostgreSQL target, event routes in `api_system_settings`, event workflow/communication cases in `api_private_contracts`, frontend feature tests and page-debug evidence. Finite design models do not replace implementation tests.

This mechanism does not promise replay, restart recovery, cross-host broadcast or exactly-once external effects. Those capabilities require a separate scope decision; observation logs must not silently become a recovery store.
