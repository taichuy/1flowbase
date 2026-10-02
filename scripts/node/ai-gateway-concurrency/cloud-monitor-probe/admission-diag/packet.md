# Capture admission cumulative timing packet

## Decision to answer

Locate elapsed time inside the existing lossless client capture owner, while
measuring the diagnostic's own disturbance. This is an observation-only packet,
not a scheduling optimization or a resource-gain claim.

The observed same-binary baseline client first-to-last-delta interval exceeded
the existing provider ingress interval. Source shows a possible downstream wait:

`ObservedBody -> record/send.await -> bounded mpsc(8) -> sole archive worker ->
64KiB/20ms raw batch -> repository archive commit/receipt -> replay by capture ID
and committed cursor (32 frames) -> decode -> classifier -> each fact's existing
append transaction/flow sequence fence/commit`.

The same worker receives and projects; request classification can occupy it while
response admission waits. This is a source-supported possibility, not measured
causality. The synthetic fixture has128 history entries/512KiB; these are neither
all real requests nor product business capacities.

## Immutable controls

- Protected semantic baseline60308d4fe3b85b6317653d5c6bda834609f4ad3a
- Diagnostic source5e3afcefd5cf6c3e09b345496b5956e30426cc40
- Diagnostic API treee571d1545ffec8ac957aa2abecc55c029901d36e
- Only5 client_trajectory source/test files differ; storage/API-server/Kernel/
  contracts/provider owners remain byte-identical to the protected baseline
- One fresh Actions release build, Rust1.98.1, default features/allocator/codegen
- Restore the already used pinned compiler cache, never save or change its policy
- Same exact file and byte hash in OFF1/ON1/ON2/OFF2; no build between fixtures
- Existing provider997 and three immutable package hashes in freeze.json
- Artifact11242053254/run37036697535 reused only for those package bytes;
  previously compiled API binaries are not read as build/runtime input or executed
- Ordinary500ms CPU/RSS/PSS/IO sampler unchanged; denied PG fields staynull;
  no sudo/CAP/perf/maps/memory/stack/kernel setting changes

Every source/API tree/package/binary mismatch stops. Rust is built/tested/run only
on Actions. All dot-cloud tests are pure Node/Python/source/syntax checks.

## Boundaries unchanged

Queue capacity, frame copying, byte/timestamp representation, raw batching,
backpressure, record and complete awaits, committed watermarks, replay order,
scope binding, partial per-fact success, transactions, ACK/sequence/collision
checks, projection/UI and provider protocol behavior are unchanged. No data is
discarded, no queue is enlarged and no worker is split or made concurrent.

OFF has no Diagnostics allocation/new Instant/logs; it retains the once-latched
environment check and Option branches. ON is explicitly set before each fresh
owned API process starts. One fixed-size counter set per capture holds no content.
There is no additional trait/future wrapper allocation around DB calls.

## Functional gate before load

Run the same targeted test executables with explicit mode0 and mode1:

- control-plane lib client_trajectory:: filter,38 cases each mode
- api-server lib client_observer::_tests filter,3 cases each mode against the
  owned ephemeral PostgreSQL fixture
- Expected82 successful case executions, zero failed/ignored; every required
  case's passing name is checked, not only Cargo exit status or a green workflow
-12 new tests cover arithmetic/saturation/cancellation/snapshot whitelist/OFF
  no-state/raw cleanup/projection failure/backpressure/actual record and complete
  future abort/concurrent durable complete waiters. Abort tests handshake after
  the actual inner future is first polled Pending, not just a scheduler yield
- Existing tests retain exact bytes/Unicode/NUL/fragments, long history, provider
  schema/tool/result semantics, pre-bind/drop, partial failure, committed replay
  watermarks and bounded admission

The first frozen run37060500298 compiled/executed38 OFF cases:37 passed,
including all12 new diagnostics cases; the existing burst comparator failed1->1.
No ON/API/build/resource phase ran. Its original fixture bytes exactly match the
historical739c.../92a62c6f3 source. Reuse that cfg(test)-only immediate-control
synchronization:0ms control waits for MemoryWriter archive notification before
next frame;20ms burst still yields. Keep every original strict comparative,
receipt256, original-byte/kind/order/sequence predicate. This defines the immediate
control's arrival-after-commit boundary, not a same-arrival performance experiment.
It does not change production zero/20ms behavior. On success or failed gates EXIT,
restore the exact original test source and require a clean product tree before
release build. Overlay/recovery hashes and diff are retained as artifact receipts.

Source contracts preserve21 original awaits and one worker spawn. These source
checks do not substitute for Rust compilation/behavior. Existing unaffected
storage evidence is reused only for unchanged storage owners. This diagnostic
does not replace the full relevant protocol merge gate and does not produce aPR.

## Fixed request/measurement budget

Four serialC8 fixtures: OFF1, ON1, ON2, OFF2. Each performs8 full-history gateway
warm requests,8 warm Native-status reads,8 measured gateway requests,8 direct
before,8 direct after and8 measured Native-status reads. Total192 primary
synthetic HTTP requests (32 measured gateway;16 per mode), excluding only owned
setup/bootstrap/cleanup management HTTP. Real model requests0, implicit retries0.
No new long waits, no C16/C32 expansion, same existing30s initial/30s warm settle,
10s idle and5s final quiet. Sampling step12min, combined gate/build90min, job110min.

One existing timing-only SELECT per fixture executes strictly after whole:end,
restricted to the current qadb/current application/exact8 measured UUIDs.
Statement timeout3s, command timeout5s and output512KiB are diagnostic safety
bounds. No fullmetrics/body/credentials/account/input are selected. Missing host
summary is explicitly unavailable and fails the diagnostic, never fabricated.

Collector attaches only to the single owned synthetic API stdout/stderr, keeps
separate partial buffers, discards unrelated lines and stores only UUID/fixed
numeric-counter payloads. Export after whole:end includes only the8 measured
flow UUIDs and their capture IDs.128 events/8KiB line/4KiB payload bounds are
diagnostic budgets, not product business limits. Missing, foreign, duplicate or
failed measured capture facts stop. OFF must emit zero diagnostic records.

## Outputs and units

raw artifact: manifest.json, summary.json, OFF1/ON1/ON2/OFF2 result.json,
samples.jsonl and already-redacted capped service logs. Build artifact contains
exact gate-counts.json/logs/source-proof.json/toolchain.txt/binary-provenance.json,
one release API binary and verified official package receipts/bytes. No auth state.

Each capture emits at most worker_end and the first complete_end, containing only
capture/flow UUIDs, fixed event labels, boolean result, clock-read count and13
stage count/total_ns/max_ns/failures/cancelled counters:

- request, response-SSE and response-JSON admission
- archive collection, archive repository commit, replay read, decoder
- request and response classifier inclusive
- each fact append, unbound discard, complete wait, worker total

Retain existing six client clock anchors, provider host summary and sampler
resources, driver CPU, diagnostic clock/log/export bytes and query/export cost.
Report each fixture separately and paired OFF/ON aggregateCPU seconds/request,
TTFT, first-to-last delta, terminal-to-body-end and total latency. Report average
single-core CPU, actual sampled peak interval/window, RSS/PSS, SQL/WAL/processIO
and file allocation with missing fields and finite-sample boundaries.

## Interpretation and stop

All stage values are monotonic elapsed wall durations. Classifier includes fact
awaits; worker total includes waiting/collect/replay/classifier/facts; admission
and complete wait overlap the worker. Do not sum them or call them CPU. Admission
starts after original bytes copy/timestamp and is send-await scheduling/admission,
not durable DB ACK or a pure queue-lock wait. classifier/decode/collect "success"
means the unit function returned; nested persistence errors live in fact_append
and worker/complete outcome. Cancelled includes any unfinished Drop/unwind; an
unpolled future has no span.

worker_end emits after stopped/notify, so either log may arrive first. Independent
atomic snapshots are not jointly atomic. Body Drop repeats complete even after
normal EOF; first complete_end is not final totals for all completion callers.
Never subtract two snapshots to invent an HTTP tail or require identical wait
counters. Server inner EOF/projection-Kernel/dispatcher/socket-flush timestamps
remain unobserved; recorder complete is an elapsed span, not a client timestamp.

Atomics, serialization, scope snapshot and two info logs add real ON overhead,
some outside the timed spans. Pipe parsing also consumes driver CPU. The same
binary OFF/ON controls measure this rather than claim zero overhead. With16
requests/mode in only two fixture clusters, small/inconsistent differences cannot
establish a precise overhead bound or production benefit. Large/inconsistent
disturbance makes localization inconclusive. Green proves bounded execution,
not optimization.

Only after actual waits and self-cost are measured may the next packet propose a
single semantic-preserving scheduling change. Do not delete awaits, batchStep+
sections transactions, add concurrency or change queue/business capacities.
Root coordinates the single dispatch after publication; never dispatch twice.
