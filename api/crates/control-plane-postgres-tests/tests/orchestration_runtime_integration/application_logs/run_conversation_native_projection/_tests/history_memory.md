# History body memory diagnosis (HM-01)

This opt-in fixture uses the real PostgreSQL repository, formal migrations and
an automatically owned temporary database/schema. Use only a dedicated fixture
PostgreSQL maintenance URL supplied explicitly as `DATABASE_URL`. It does not
read development environment files or contact an application server.

Build `control-plane-postgres-tests --test orchestration_runtime_integration`
with `--features history-memory-probe --no-run --locked`. Resolve `--jobs` from
`node scripts/node/testing/verify-runtime.js cargo-jobs`, using the normal Cargo
resource wrapper and this worktree's target. The feature is Linux GNU only.

Run the saved test executable with:

```text
--ignored --exact application_logs::run_conversation_native_projection::incremental::history_memory::history_memory_phase_probe --nocapture --test-threads=1
```

Required environment: an explicit owned `DATABASE_URL` and a fresh
`HISTORY_MEMORY_OUT`. Select a mode with `HISTORY_MEMORY_MODE`:

- `performance` (default): `_RJEM_MALLOC_CONF=prof:false`. No canary, native
  profile dumps, sampler thread, backtrace warmup/capture, diagnostic row holder
  or full-history/UI oracle. Identical atomic requested-live/peak/allocation/free
  counters remain in baseline and candidate. Phase endpoints record allocator
  stats, current RSS/anonymous memory and process CPU ticks. Timers cover only
  the repository append, before those endpoint samples and return assertions.
- `proof`: also `prof:false`; separate full-history/UI and query-equivalence
  phases run after measured appends/pool close. Their memory is not append cost.
- `stack`: `prof:true,prof_active:true,lg_prof_sample:17,prof_accum:false`;
  enables native profiles, 2ms sampling and prewarmed exact-peak backtraces.
  This is ownership diagnosis, not comparable RSS/latency evidence.
- `canary`: the same profile configuration as `stack`, but a standalone process
  runs only a touched/freed 32MiB allocation and exits without a database. Never
  run calibration before performance appends in the measured process.

Always use the same mode/configuration/fixture source for paired measurements.
The defaults are `HISTORY_MEMORY_N=1024`, `HISTORY_MEMORY_ITEM_BYTES=16384`,
`HISTORY_MEMORY_ROUNDS=20`, C1 and delta1. Set
`HISTORY_MEMORY_BUFFER_CONTROL=1` for the fixed 16 MiB write-only/read-only
connection capacity control. Do not set `HISTORY_MEMORY_ROLE` at the outer call;
the coordinator supplies that to its sequential child processes.

The coordinator alone seeds the framework and owns database cleanup. Seed A
materializes history and appends it once; A exits before consumer B starts. B
appends delta1 for 20 rounds, closes its measured pool, then performs the complete
persisted-original/UI oracle on a new connection. Return assertions and the
full-history oracle have separate phase IDs (the latter is proof/stack only).
The optional buffer control is a third process. Each measured process records
PID/start ticks/executable identity, phase requested-live/peak counters, cumulative
requested allocation/free counters, jemalloc allocated/active/resident/mapped/
retained, current RSS/anonymous memory, lifetime HWM, and idle endpoints.
`HISTORY_MEMORY_ONLY_BUFFERS=1` reruns just that control under the same isolated
database owner without repeating the history experiment.
`HISTORY_MEMORY_ONLY_SEED=1` repeats only the seed when exact transient peak
attribution is needed. In stack mode, requested-live peaks above 96 MiB retain a
prewarmed GNU libc backtrace in fixed atomic slots. This diagnostic threshold
does not limit allocation or content. `peak_stack_live` must match the measured
hook peak before treating its stack as that peak's owner; polling heap profiles
alone cannot establish that association. No stack capture attaches to a service.

`HISTORY_MEMORY_CONTEXT_CONTROL=1` instead runs the retained-context orthogonal
control: one stored 16KiB output and one new 16KiB output in each case, with one
valid `function_call_output` tool result of 1KiB, 16MiB, or 16MiB with a NUL that
uses the supported lossless sidecar. Progress must already be published before
each fresh consumer. Its manifest contains only run UUIDs; no `get_flow_run`
loads context during setup. The NUL sidecar takes the existing full-repair path
and is interpreted separately from the plain incremental pair. The original
N1024 seed experiment is an initialization/replay case; stream persistence has
an existing 64KiB JSON-byte flush trigger and 20ms interval, with oversized single
events still allowed. It is not evidence of a typical append batch. `large-c2`
adds two overlapping appends for independent run IDs in one consumer process,
with two preconnected physical SQLx connections. The fixture asserts interval
overlap and records both intervals/backend IDs and one combined process peak;
it does not sum two independent process RSS values.

In proof/stack mode, after the real append a new connection executes the frozen
71ff baseline SQL, actual compiled production SQL and an independent exact28
projection in one repeatable-read/read-only snapshot. `query-contract.json`
records all three SQL sources; extraction accepts whitespace before the actual
production string literal. `row-bytes.json` reports actual `PgValueRef` bytes per column,
including duplicate-name ordinals, and verifies type/format/null/value bytes for
all mapper-selected fields. Unused `log_context`/sidecar JSON is not decoded by
that row probe. Decoded JSON key/string UTF8 bytes and hook/OS memory are distinct
metrics; PostgreSQL JSONB processing is not app allocation. No production query
is replaced by this control. `alias-sentinel.json` uses differing physical JSONB
and NUL-restored JSON values with the same name, asserting that SQLx name lookup
selects the final restored alias. This proof is independent of same-value rows.
`HISTORY_MEMORY_CONTEXT_CASE_ONLY=large-sidecar` selects only that bounded
sidecar control when the plain pair already has reusable evidence. `small`,
`large` and `large-c2` select the other individual cases. The sidecar case keeps
the full16MiB corpus of the original setup OOM; choose the owned fixture PG budget
from current available memory and the existing verification budget. Preserve
the original1GiB cgroup OOM as a failed setup, distinct from a later retry.

The Rust counter wraps the actual prefixed/static Jemalloc GlobalAlloc. It counts
requested layouts; it does not count jemalloc size rounding, native profiling
metadata or transient internal realloc storage. Native `.heap` files contain
sampled live call stacks, not exact requested bytes. Use the same executable with
`jeprof --text --show_bytes EXECUTABLE PROFILE`. Validate that the canary stack is
present while held and absent after release before interpreting any empty stack.
The polling profiles may lag a hook peak. Current RSS and allocator resident
need not decrease immediately after logical frees; HWM is never a phase peak.

`shrink_buffers()` is used only on this fixture's owned connection to attribute
retained memory. It is not part of the proposed product change. The normal
`history_memory_contract` test preserves exact returned/persisted bodies, edge
JSON shapes, identities/order, projection fields and rollback after a controlled
projection write failure. Keep the existing `append_regressions` oracles as well.
Compiling this target is not acceptance of that regression test; Root's single
fresh QA runs behavior acceptance on the frozen candidate.
