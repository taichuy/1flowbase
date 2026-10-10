# Runtime Extension Backend boundaries

[中文](runtime-extension-backend-evolution.md)

## Current architecture

The open-source edition has one Backend container and one `api-server` process. `api-server` is the composition root and creates one in-process `RuntimeExtensionHost`. The Host exclusively owns active Provider, DataSource, Capability and Network Egress registries, worker processes, framed stdio, Runtime Profile and lifecycle state.

Control Plane and Orchestration do not construct concrete Hosts. Orchestration resolves targets; its Backend adapter holds only `RuntimeExecutionPort`, owned by `runtime-core`. `RuntimeObservationPort` exposes Host snapshots and is not a dependency of that adapter. API business paths use typed `ProviderRuntimePort`, `DataSourceRuntimePort`, `CapabilityRuntimePort` and `NetworkEgressRuntimePort`, not concrete registries. These six Ports form `RuntimeBackend`; `RuntimeBackendSlot` binds exactly one backend. The open-source binding is the in-process Host.

## Stable Ports

`RuntimeExecutionPort` provides typed execution and provider-distribution operations:

- `activate_provider_distribution_rule` / `deactivate_provider_distribution_rule`: activate or deactivate a distribution-rule plugin.
- `select_provider_distribution`: accept a typed distribution request and return a selection receipt; business routing policy remains with the caller.
- `execute`: one request and one terminal outcome.
- `execute_stream`: required and diagnostic events for the same request.
- `cancel`: cancel a Host-managed active task by `request_id`.

`RuntimeObservationPort::snapshot` reports lifecycle, registry counts and active requests. The four business Ports require typed methods per operation, without default `UnsupportedOperation` implementations. Package activation accepts only `RuntimeArtifactReference`. The composition root injects a resolver that maps installation IDs to local materializations; filesystem paths do not enter stable Ports. Ports expose no HTTP/gRPC, ports, processes, filesystem paths, registries or unbounded universal JSON calls.

The `runtime-extension-host` crate root exposes `RuntimeExtensionHost`, `RuntimeArtifactResolver` and explicitly approved stable facades. Concrete Hosts, registries, workers, stdio, loaders and supervisors are private. Tests of internals belong under `src/_tests`; external tests use facades or the six Ports.

`request_id` identifies one Host call. Legacy wire does not carry it; multiplex carriers allocate strictly increasing `call_id` values within each worker incarnation to correlate messages without changing business identity. Callers provide a traceable unique identity for each logical attempt. Ports do not promise duplicate-execution idempotency; business contracts and Control Plane own any required idempotency keys.

## Lifecycle and failures

```text
Discover -> Validate -> Compile Graph -> Select Backend
-> Reconcile Packages -> Ready
-> Execute -> Drain -> Stop
```

The Host enters Ready after package reconciliation during Starting. Ready does not imply every plugin process has started or stays resident. Draining rejects new requests and cancels Host-managed active tasks. Stop stops registry workers and reaches Stopped. Startup, execution, cancellation and shutdown failures map to stable `RuntimeBackendError` categories, never fabricated remote-service unreachability.

Timeouts are caller policy; cancellation is Host execution responsibility. Legacy plugins retain Host-task/worker-operation cleanup. Workers explicitly declaring `stdio_json_multiplex_v1` receive correlated cancellation; the SDK acknowledges only after the task actually terminates. The Host retains reservations until a terminal result or confirmed process exit. Cancellation acknowledgement is not an upstream WebSocket peer-close ACK; existing transport-closure contracts own that evidence.

System status preserves the `services.plugin_runner` JSON key for consumers, but its service authority is `runtime-extension-host`. API and Host profiles share a PID, so Host-group memory/process counts are sampled once. A Host profile failure fails the status request rather than pretending a remote service is unreachable.

## Workers and protocols

Published manifests support `process_per_call`, `stateful_provider_worker`, `stateful_runtime_worker`, and `stdio_json`, `stdio_json_worker`, or explicit opt-in `stdio_json_multiplex_v1`. The Host owns startup, readiness, execution, drain and stop/failure states. `extension-contracts` owns method/event/error/result and graceful-exit semantics. The Host does not own business Provider Routing, authorization, transactions, installation or signing decisions.

Multiplex reuse is bounded by the loaded artifact; reload/unload fences the old incarnation. Logical sessions may share a process, but calls within one session remain serial. The carrier owns call correlation, byte backpressure, cancellation and exit. Typed provider results/events and trusted PluginData bindings remain unchanged. Closing one session cannot terminate neighbors' shared process. Plugins retain short-lived connection/session state needed for recovery, not cached copies of durable long-term context.

Shared-worker admission observes actual RSS, system MemAvailable and cgroup v2 ancestor limits, reserving request/stream buffers not yet reflected in RSS. Already-observed memory growth must not be deducted repeatedly per concurrent call. Fixed buffers are reserved per process, not as a whole process allowance for every session. Estimates are not physical-memory guarantees; manifest `memory_bytes` remains independent address-space protection. Six shared official providers no longer declare the former 256 MiB RLIMIT_AS unsuitable for shared concurrency.

## Provider worker demand and idle reclamation

The Host obtains or starts provider workers for actual calls. Installation, selectability and process liveness are separate states. Control Plane supplies revisioned demand; older revisions cannot overwrite newer state. The Host uses Tokio `DelayQueue` for idle deadlines, with a default 90-second grace period. Explicitly unselectable plugins skip that grace once idle conditions hold. This is process-reclamation policy, not a session-count or business-capacity limit.

On expiry, the Host rechecks incarnation and idle revision; supervisor admission/binding state determines whether quiescing is safe. Timers cannot infer transport release or terminate workers still carrying valid calls or bindings. Existing lifecycle evidence governs exit and cleanup. Host/SDK own dispatch, cancellation, notifications and reclamation; plugins do not each implement a supervisor protocol.

Implementation: [execution Port](../../api/crates/runtime-core/src/runtime_backend.rs), [Orchestration adapter](../../api/crates/orchestration-runtime/src/runtime_backend.rs), [idle scheduler](../../api/crates/runtime-extension-host/src/provider_host/idle_workers.rs).

## Limited Rust SDK

`runtime-extension-sdk` provides a typed PluginData client, Host Simulator and golden fixtures for `runtime_host_call/v1`, plus a Tokio async dispatcher for multiplex stdio. Internal dependencies close only over `extension-contracts`; official plugins consume the same SDK/wire at an exact Git revision. It excludes manifest construction, Provider Routing, Host Registry, Control Plane, Domain, database and `api-server` types. TypeScript/Python SDKs, generic handler registration and a full Conformance Kit require separate delivery.

## Future remote adapter

Remote protocols, remote SDKs and clusters are not implemented in this phase; the limited local Rust SDK above exists. A future trusted HostExtension may contribute `RemoteRuntimeClusterAdapter` only by replacing the single `RuntimeBackendSlot` binding at the composition root. This must not require changes to Control Plane, Orchestration or business routes. It resolves the same `RuntimeArtifactReference`, not a local path supplied by business code. Discovery, retries, circuit breakers, leases, elections, scheduling and scaling require separate contracts and delivery, not premature exposure in stable Ports.

Every Backend must implement all six Ports at compile time. Partial-capability backends require a separate explicit capability set and bind-time validation; do not restore default failures or runtime capability probing in business paths.

Official compatibility gates must build and launch eight official executables through the real Host, covering all three execution modes, old/new stdio, Validate/CountTokens/Generate events/errors/results and the Clash stateful Network Egress worker. Manifest-only checks do not establish runtime compatibility.

Stop and reassess if work introduces a second working Backend or Runtime Profile authority, leaks transport/process details through Ports, forces plugin wire changes, or takes over routing, authorization, transaction or installation decisions.
