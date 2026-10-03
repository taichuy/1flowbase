# Gateway resource benchmark

Reusable bounded Linux/Node benchmark for an **already built** API and supplied official package archives. It creates the existing gateway fixture on a free loopback port, routes real Responses history through the OpenAI plugin to a controlled mock, and owns only that API, mock, and temporary console session. Database creation, reset, clone and deletion belong to the caller. Use a new dedicated disposable database for each comparative run; bootstrap inserts data and existing root configuration is not reused. No builds, package downloads or DB lifecycle commands run here.

Run `node scripts/node/ai-gateway-concurrency/resource-benchmark/run.cjs` with these environment variables. Provide secrets through the environment of a private launcher; do not print environment or put credentials in argv/artifacts.

| Variable | Meaning |
| --- | --- |
| `RB_REPO_ROOT` | Absolute source root containing existing gateway-fixture and page-debug/auth.js |
| `RB_API_BINARY` | Existing executable API binary |
| `RB_OPENAI_PACKAGE`, `RB_ANTHROPIC_PACKAGE`, `RB_OPENAI_COMPATIBLE_PACKAGE` | Existing archive paths; no implicit latest/default packages |
| `RB_DATABASE_URL` | Dedicated disposable PostgreSQL URL; fixture requires database name beginning qadb/fixture/test/tmp/temp |
| `RB_DEDICATED_DATABASE=1` | Caller acknowledgment that this DB is exclusively allocated to this run |
| `RB_ARTIFACT_ROOT` | New directory below `RB_REPO_ROOT/tmp/test-governance/`; existing report is refused |
| `RB_HISTORY_BYTES` | 8192 (default), 131072, 524288 |
| `RB_CONCURRENCY` | 1 (default), 8 |
| `RB_WARMUPS`, `RB_ROUNDS` | Warmup rounds 0–5 (default 1), measured rounds 1–10 (default 3) |
| `RB_IDLE_MS`, `RB_DRAIN_MS` | Separate idle and per-round drain window, 500–10000ms (default 3000 each) |
| `RB_SAMPLE_MS` | 50–1000ms (default 100) |
| `RB_API_PORT` | Optional explicit free loopback port; 7600 and 7800 refused |
| `RB_DATABASE_PID` | Optional host-visible external DB process PID; permissions/not visible => null evidence |
| `RB_PG_MODULE` | Optional absolute path to an already installed `pg` module; default `require('pg')`. No install. |
| `RB_DEDICATED_CLUSTER=1` | Enable optional cluster-wide WAL LSN observation only for exclusive cluster |

The fixture inherits normal parent environment, including a caller-supplied `API_PLUGIN_UPLOAD_MAX_BYTES` for known package sizes. Keep upload budget equal across runs. The benchmark uses injected OwnerHttpClient backed by `page-debug/auth.js` temporary owner; session revocation precedes fixture shutdown in `finally`.

Mock traffic is fixed at 256 text deltas separated by 10ms, with deterministic text and existing Responses event shape. History is 128 role=user/input_text messages, with exact UTF-8 **text** byte budget; JSON overhead is separately recorded as request_bytes. Every request must return exactly 256 deltas, once-completed status, matching concatenated/terminal text SHA-256 and no failure/cancel/incomplete/error event. Warmups have separate request records and do not enter workload CPU/PSS measurement. Measured rounds record workload and drain independently. Upstream request count and aggregate wire payload bytes detect unexpected retries. Output contains no payload text, credentials, cookies or copied env. Exceptions retain name/code and a truncated canary-redacted message.

`report.json` contains binary/package SHA-256 provenance, bounded samples, API/plugin CPU seconds, RSS/PSS sampled peaks, harness CPU, bracketed sampler CPU/wall overhead, baseline idle, workload/drain windows, response latency and request/output oracle. `service/` contains existing fixture sanitized logs. Stdout is one small outcome object. Exit 0 means observations collected and cleanup succeeded; it is not a performance acceptance verdict. Failure reports retain failed phase and sanitized diagnostic name/code/message. ready.json is a safe post-bootstrap receipt with gateway PID/start_ticks, loopback port and binary hash for an external process owner guard. SIGINT/SIGTERM requests stop at the next bounded phase boundary and cleanup; requests have 30s timeouts. Hard kills cannot execute JavaScript finally.

`getconf CLK_TCK` converts `/proc/<pid>/stat` ticks. Process identity is `(pid,start_ticks)`. Every `/proc/<pid>/task/<tid>/children` is traversed. Missing children, process disappearance, PID reuse or counter regression invalidate the affected total (null). Matched observations also expose a lower bound; short-lived processes may escape sampling. Every API descendant is classified as plugin; this taxonomy is only valid when the fixture has no unrelated child services. RSS and PSS sums are separately computed inside each sequential sample before taking peaks; sample timestamp/capture duration remain visible. Do not sum individual process peaks. Unreadable/missing PSS remains null. External DB PID is the root of a separately enumerated database process tree. API/plugin/database completeness is tracked independently; inaccessible DB children invalidate the DB total while API evidence stays usable. RSS falls back to readable VmRSS when smaps_rollup is denied; PSS remains null.

With installed `pg`, or `/usr/bin/psql` fallback using credentials exclusively in PG* env and bounded fixed SQL, snapshots outside measured windows report database bytes and user-table heap/index/TOAST bytes. TOAST includes its indexes; heap/index exclude TOAST to avoid double counting. Temporary/system relation sizes are not included in table breakdown. Missing observation tooling or DB permission yields nulls with a fixed reason and sanitized diagnostic. No dependency installation runs. WAL stays null without dedicated-cluster acknowledgment and remains null when inaccessible. WAL LSN differences measure cluster emission, not fsync, disk footprint or per-database WAL attribution.

Draining is a bounded observation window, **not proof of completed background work**. It reports end-window mock counters and `completion_claim:false`. Sampling can miss bursts. Harness CPU includes client, mock and sampler; sampler brackets separately report observer overhead without treating subtraction as exact target CPU correction. Compare identical dimensions, packages, binary profile, database seed and environment across repeated runs. Binary hash binds the executable artifact; source/build-profile attribution still requires caller evidence.

Pure tests are in `_tests/core.test.cjs`. Central QA command: `node --test scripts/node/ai-gateway-concurrency/resource-benchmark/_tests/core.test.cjs`. They reject PID reuse/regression/incomplete thread children, falsely summed PSS peaks, secret-bearing artifacts and partial/wrong/duplicated completed output. Implementation handoff only runs `node --check`; runtime and acceptance evidence belong to Root's central QA.
