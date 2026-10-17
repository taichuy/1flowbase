# Plugin lifecycle contracts

[中文](plugin-lifecycle-contracts.md)

Synchronous control, committed facts and final outcomes are separate contracts:

```text
Effective Graph -> frozen typed Hook Plan
  -> Authorize -> Admit -> Before -> Invoke -> After / Failure -> Completion

Transaction owner -> frozen subscriber plan + durable fact + subscriber rows
  -> commit -> independent handler delivery -> ACK after successful handling
```

## Ownership

- `extension-contracts` owns typed Hook metadata, facts, outcomes, commands and diagnostics.
- `plugin-framework` compiles ordered plans and the handler registry. The composition root injects active HostExtension native factories.
- `interface-runtime` executes projected plans without depending on the graph compiler, storage or Control Plane.
- Each domain owns its decisions: authorization Deny is absorbing; constraints use the domain's safe intersection.
- The PostgreSQL transaction owner commits the fact and subscriber rows atomically. Subscribers cannot change an already committed operation.
- `runtime-extension-host` owns managed subprocesses, message dispatch, call correlation, cancellation, crashes and exit cleanup. `runtime-extension-sdk` supplies shared typed protocol handling for plugin authors; each plugin does not maintain its own Host delivery protocol.

## Request, plugin and event boundaries

The AI Gateway retains its protocol mapping → AI Native → provider layers. The Interface Kernel supplies shared invocation governance and executes frozen authorization and admission plans; domains retain their business policies and transactions. The Effective Graph declares valid contributions and compiles plans. It does not own subprocess runtime state or business completion.

| Channel | Guarantee and completion boundary | Owner |
| --- | --- | --- |
| Request streams | Preserve protocol terminal state, connection handling and Completion; no generic Outbox conversion | AI Native and protocol adapter |
| Ephemeral EventBus | In-process FIFO per topic, complete JSON and destructive take; no restart recovery or subscriber ACK guarantee | `storage-ephemeral` |
| Reliable business notifications | Atomically commit business changes, facts and frozen targets; deliver and ACK each target independently | Transaction owner and Lifecycle Outbox |
| Request Hooks | Execute frozen phase permissions; do not stand in for asynchronous subscriber ACKs | Kernel and typed Hook owner |

Invocation completion, business commit, handler success, durable ACK and client delivery require separate evidence. Notification failure cannot rewrite the original invocation result or provider error. A subscriber changes subsequent business state through a new typed command, never by changing the original invocation's terminal state.

## Reliable delivery and recovery

The frozen subscriber plan, business changes, fact and per-target deliveries commit together. Rollback leaves no delivery obligation; the Dispatcher observes the work only after commit. Existing frozen targets take precedence over the current subscription graph. Disabling or upgrading a plugin cannot rebind an old target to a new implementation.

Outbox attempts may repeat. `SKIP LOCKED`, concurrent handlers and retries do not preserve completion order across events. A business contract that requires ordering must define its own object versions or ordering constraints; claim selection order does not imply global FIFO.

`claim_id` and leases fence claims and ACK state only. Effect deduplication uses stable event and subscriber identities and commits atomically with the effect. External side effects require the actual receiver's idempotency protocol. Fencing does not imply network-wide exactly-once execution.

The current Dispatcher's batch size, lease, deadline and retry budget are scheduling policies. Exhausting the budget or losing execution authority pauses delivery while retaining the fact. Managed plugins can resume an exact delivery through the extension center after validation of the original frozen target, executable artifact and current authority. Paused work is not delivered; a resume endpoint cannot guarantee eventual delivery after permanent authorization revocation.

## Ephemeral data and legacy retirement

The default EventBus directly uses `MemoryEventBus::lossless()`. Publishing, taking and inspecting events do not query a historical PostgreSQL queue. Payload inspection retains its separate reveal contract; inspection budgets do not become business message-size limits.

Published orchestration startup plans that are cheap to rebuild and reused soon use the existing approximately five-minute cache. Long-lived persisted context is fetched by database key or index instead of being copied wholesale into ephemeral storage. Cache eviction budgets and business capacity limits are separate decisions.

The development-era Legacy Ledger and internal dual-version publication dispatch leave the runtime. The original `model_definition.committed@v1` domain contract and body remain; only the internal handoff version, old queue and recovery implementation are removed. A new forward migration requires explicit disposition to empty the old queue and no unfinished old model deliveries. Failure leaves the retirement schema change unapplied. Applied SQLx migrations are not rewritten.

This stage does not support historical external backup formats containing the old queue. Current-version backup creation, validation and restore retain source facts, frozen targets and valid receipts. Current backups contain neither the Legacy Ledger nor internal legacy publication fields.

Performance evidence separates stored bodies, rows and indexes from WAL/IO per interval and CPU/latency per request. Removing an old queue does not automatically speed up Gateway requests that never used it.

## Boundaries

Hooks cannot read credentials or bypass authentication, authorization, transactions, audit or domain state machines. After, Failure and Completion observe; they cannot turn failure into success. Subscribers do not synchronously control the original invocation; subsequent changes require a new typed command. There is no arbitrary SQL, database connection, string-dispatched Hook, universal JSON decision or plugin-defined aggregation function.

Each invocation freezes the registry and graph fingerprints. Delayed delivery uses the original graph, subscriber and handler version; a missing matching handler fails closed instead of switching to the current version. Each subscriber has its own claim, fencing, retry and idempotency boundary. The fact becomes Delivered only after every subscriber succeeds. No active subscribers means no Outbox write.

Trusted HostExtensions bind BootSnapshot / Invocation handlers through `native.library` and `native.entry_symbol`; missing factories, bindings or matching contracts fail at startup. RuntimeExtension and CapabilityPlugin subscribers use the Host's managed transport for the supported Create committed contract or declared namespaced events. The current event-subscriber implementation supports Invocation / WorkspaceAssignment; this is not a permanent scope definition for all managed contributions (see [plugin selection](plugin-composition.en.md)). Subscribers require matching contract/version and contribution-level `event.subscribe` authorization. User scope is not open; authentication factories remain trusted Host responsibilities. Successful compilation does not replace execution-time authorization.

The delivery adapter consumes frozen plans and injected handlers, never hard-coded plugin behavior. Enqueuing an event is not handler completion. The handler deadline must precede claim expiry; timeout yields TimedOut and retry while other subscribers can proceed.

## Managed interface phase permissions

Kernel typed Hook capabilities do not grant ordinary managed plugins the same wire permissions. The Host validates outcomes by phase; returning Deny cannot grant control that the protocol does not expose.

| Phase | Generic managed interface (schema frame v1 / reference v2) | Legacy Create adapter |
| --- | --- | --- |
| Authorization / Admission | Continue or veto; cannot reverse a core denial | Original continue/veto contract |
| Before | Observe only; no input patching or veto | Read-only veto under the original contract; no input patching |
| After / Failure / Completion | Observe only; failures cannot replace the primary result | Observation only; preserve the primary result |

Any input mutation in other trusted typed contracts requires explicit authority from that contract, not inference from Create. See the [invocation lifecycle](interface-lifecycle.en.md#execution-and-extensions) for ordering.

## Evidence

Deterministic fixtures separately cover contracts, graph compilation, Kernel order/terminal and Outbox commit/rollback. Dependency checks prevent `interface-runtime` from depending on `plugin-framework`, storage, Runtime Host or protocol adapters. Actual handlers, transactions and protocols must be verified against a frozen candidate; historical Root completion is not evidence for today's code.
