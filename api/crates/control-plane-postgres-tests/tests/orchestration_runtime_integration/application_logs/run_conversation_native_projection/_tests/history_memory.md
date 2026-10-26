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

Required environment: `DATABASE_URL`, a fresh output directory in
`HISTORY_MEMORY_OUT`, and
`_RJEM_MALLOC_CONF=prof:true,prof_active:true,lg_prof_sample:17,prof_accum:false`.
The defaults are `HISTORY_MEMORY_N=1024`, `HISTORY_MEMORY_ITEM_BYTES=16384`,
`HISTORY_MEMORY_ROUNDS=20`, C1 and delta1. Set
`HISTORY_MEMORY_BUFFER_CONTROL=1` for the fixed 16 MiB write-only/read-only
connection capacity control. Do not set `HISTORY_MEMORY_ROLE` at the outer call;
the coordinator supplies that to its sequential child processes.

The coordinator alone seeds the framework and owns database cleanup. Seed A
materializes history and appends it once; A exits before consumer B starts. B
appends delta1 for 20 rounds, closes its measured pool, then performs the complete
persisted-original/UI oracle on a new connection. Return assertions and the
full-history oracle have separate phase IDs. The optional buffer control is a
third process. Each measured process records PID/start ticks/executable identity,
a touched/freed 32 MiB canary, phase requested-live/peak counters, cumulative
requested allocation/free counters, jemalloc allocated/active/resident/mapped/
retained, current RSS/anonymous memory, lifetime HWM, and idle endpoints.
`HISTORY_MEMORY_ONLY_BUFFERS=1` reruns just that control under the same isolated
database owner without repeating the history experiment.
`HISTORY_MEMORY_ONLY_SEED=1` repeats only the seed when exact transient peak
attribution is needed. New requested-live peaks above 96 MiB also retain a
prewarmed GNU libc backtrace in fixed atomic slots. This diagnostic threshold
does not limit allocation or content. `peak_stack_live` must match the measured
hook peak before treating its stack as that peak's owner; polling heap profiles
alone cannot establish that association. No stack capture attaches to a service.

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
