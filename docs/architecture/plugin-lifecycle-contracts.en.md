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

## Boundaries

Hooks cannot read credentials or bypass authentication, authorization, transactions, audit or domain state machines. After, Failure and Completion observe; they cannot turn failure into success. Subscribers do not synchronously control the original invocation; subsequent changes require a new typed command. There is no arbitrary SQL, database connection, string-dispatched Hook, universal JSON decision or plugin-defined aggregation function.

Each invocation freezes the registry and graph fingerprints. Delayed delivery uses the original graph, subscriber and handler version; a missing matching handler fails closed instead of switching to the current version. Each subscriber has its own claim, fencing, retry and idempotency boundary. The fact becomes Delivered only after every subscriber succeeds. No active subscribers means no Outbox write.

Trusted HostExtensions bind BootSnapshot / Invocation handlers through `native.library` and `native.entry_symbol`; missing factories, bindings or matching contracts fail at startup. RuntimeExtension and CapabilityPlugin subscribers use the Host's managed transport for the supported Create committed contract or declared namespaced events. Their lifecycle scopes are Invocation / WorkspaceAssignment, with matching contract/version and contribution-level `event.subscribe` authorization. User scope is not open; authentication factories remain trusted Host responsibilities. Successful compilation does not replace execution-time authorization.

The delivery adapter consumes frozen plans and injected handlers, never hard-coded plugin behavior. Enqueuing an event is not handler completion. The handler deadline must precede claim expiry; timeout yields TimedOut and retry while other subscribers can proceed.

## Evidence

Deterministic fixtures separately cover contracts, graph compilation, Kernel order/terminal and Outbox commit/rollback. Dependency checks prevent `interface-runtime` from depending on `plugin-framework`, storage, Runtime Host or protocol adapters. Actual handlers, transactions and protocols must be verified against a frozen candidate; historical Root completion is not evidence for today's code.
