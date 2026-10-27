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
| `RB_WORKLOAD` | `text` (default) or `mixed` (Responses text plus an equal number of concurrent multipart uploads/readbacks) |
| `RB_UPLOAD_BYTES` | 1048576 (1 MiB default) or 8388608 (8 MiB adverse fixture); mixed mode only consumes the bytes |
| `RB_CONCURRENCY` | 1 (default), 3, 8; requests and, in mixed mode, uploads per batch |
| `RB_DATABASE_POOL_MAX_CONNECTIONS` | Optional positive `u32` override of `API_DATABASE_POOL_MAX_CONNECTIONS` (P); absent keeps fixture P=5 |
| `RB_DATABASE_MANAGED_MAX_CONNECTIONS` | Optional positive `u32` override of `API_DATABASE_MANAGED_MAX_CONNECTIONS` (M); no injected default |
| `RB_WARMUPS`, `RB_ROUNDS` | Warmup rounds 0–5 (default 1), measured rounds 1–10 (default 3) |
| `RB_IDLE_MS`, `RB_DRAIN_MS` | Separate idle and per-round drain window, 500–10000ms (default 3000 each) |
| `RB_SAMPLE_MS` | 50–1000ms (default 100) |
| `RB_API_PORT` | Optional explicit free loopback port; 7600 and 7800 refused |
| `RB_DATABASE_PID` | Optional host-visible external DB process PID; permissions/not visible => null evidence |
| `RB_PG_MODULE` | Optional absolute path to an already installed `pg` module; default `require('pg')`. No install. |
| `RB_DEDICATED_CLUSTER=1` | Enable optional cluster-wide WAL LSN observation only for exclusive cluster |

The fixture inherits normal parent environment, including a caller-supplied `API_PLUGIN_UPLOAD_MAX_BYTES` for known package sizes. Keep upload budget equal across runs. The benchmark uses injected OwnerHttpClient backed by `page-debug/auth.js` temporary owner; session revocation precedes fixture shutdown in `finally`. Its injected spawn wrapper runs API with cwd=`repoRoot/api` so copied/CI binaries can find builtin source assets. Provider installation, MCP library and backup repository remain fixture scratch paths; application/model-pricing templates refer to real source assets. In `mixed` mode only, API cwd stays in the fixture-owned scratch directory. Static `host-extensions`, `capability-plugins` and `sets` directories are symlinked from source for builtin discovery; writable providers/MCP/backups remain private. The default business-file root then resolves to `scratch/api/storage`. Before issuing workloads, the owner reads storage/table settings and requires exactly one enabled local default storage at that exact path and an active default table bound to it (or inheriting it). Foreign persisted bindings fail the run; the harness never redirects shared business storage. Fixture cleanup removes the scratch directory and uploaded objects after session disposal; the dedicated disposable database still belongs to the caller.

The connection plan has exactly the two optional RB inputs above. Values use Rust's positive `u32` decimal syntax (an optional leading `+` and leading zeros are accepted); zero, whitespace, fractions, exponent/hex notation and values above 4294967295 are rejected. The known P+M allocation must also fit `u32`, matching W1's checked sum: validation uses fixture P=5 when P is absent and M=P when M is absent. This is a type/allocation boundary, not a new business maximum. The application still validates its complete effective settings.

The existing API `spawnOwned` wrapper applies the validated numeric plan after the fixture's default env, so P=10 actually overrides fixture P=5. Reports expose only the supplied numeric values under `configuration.connectionPlan`, keyed by their exact API env names. No plan means no new report property and no spawn overrides. This is an override receipt, not an inspection of effective application configuration. Other RB connection names, arbitrary env maps, credentials and fixture/session objects are not forwarded or published as a plan. D/I/T limits, timeouts, recycling, instance allocation and reserve are not configured by this runner; it never enables a full finite instance budget by default. Existing parent-env inheritance remains unchanged. Comparative launchers must control ambient `API_DATABASE_*` settings rather than rely on unreported inherited overrides; use the two RB inputs for P/M. An extra M override must appear in the report and be treated as a separate configuration dimension.

Concurrency 3 is an additional workload dimension. Existing 1/8 consumers retain their behavior. Root #2691's limit of three simultaneous real upstream calls is an execution scheduling constraint for that task; it does not cap this runner or product concurrency at three.

Mock traffic is fixed at 256 text deltas separated by 10ms, with deterministic text and existing Responses event shape. History is 128 role=user/input_text messages, with exact UTF-8 **text** byte budget; JSON overhead is separately recorded as request_bytes. Every request must return exactly 256 deltas, once-completed status, matching concatenated/terminal text SHA-256 and no failure/cancel/incomplete/error event. Warmups have separate request records and do not enter workload CPU/PSS measurement. Measured rounds record workload and drain independently. Upstream request count and aggregate wire payload bytes detect unexpected retries. Output contains no payload text, credentials, cookies or copied env. Exceptions retain name/code and a truncated canary-redacted message.

`report.json` contains binary/package SHA-256 provenance, bounded samples, API/plugin CPU seconds, RSS/PSS sampled peaks, harness CPU, bracketed sampler CPU/wall overhead, baseline idle, workload/drain windows, response latency and request/output oracle. `service/` contains existing fixture sanitized logs. Stdout is one small outcome object. Exit 0 means observations collected and cleanup succeeded; it is not a performance acceptance verdict. Failure reports retain failed phase and sanitized diagnostic name/code/message. ready.json is a safe post-bootstrap receipt with gateway PID/start_ticks, loopback port and binary hash for an external process owner guard. SIGINT/SIGTERM requests stop at the next bounded phase boundary and cleanup; requests have 30s timeouts. Hard kills cannot execute JavaScript finally.

`getconf CLK_TCK` converts `/proc/<pid>/stat` ticks. Process identity is `(pid,start_ticks)`. Every `/proc/<pid>/task/<tid>/children` is traversed. Missing children, process disappearance, PID reuse or counter regression invalidate the affected total (null). Matched observations also expose a lower bound; short-lived processes may escape sampling. Every API descendant is classified as plugin; this taxonomy is only valid when the fixture has no unrelated child services. RSS and PSS sums are separately computed inside each sequential sample before taking peaks; sample timestamp/capture duration remain visible. Do not sum individual process peaks. Unreadable/missing PSS remains null. External DB PID is the root of a separately enumerated database process tree. API/plugin/database completeness is tracked independently; inaccessible DB children invalidate the DB total while API evidence stays usable. RSS falls back to readable VmRSS when smaps_rollup is denied; PSS remains null.

With installed `pg`, or `/usr/bin/psql` fallback using credentials exclusively in PG* env and bounded fixed SQL, snapshots outside measured windows report database bytes and user-table heap/index/TOAST bytes. TOAST includes its indexes; heap/index exclude TOAST to avoid double counting. Temporary/system relation sizes are not included in table breakdown. Missing observation tooling or DB permission yields nulls with a fixed reason and sanitized diagnostic. No dependency installation runs. WAL stays null without dedicated-cluster acknowledgment and remains null when inaccessible. WAL LSN differences measure cluster emission, not fsync, disk footprint or per-database WAL attribution.

Draining is a bounded observation window, **not proof of completed background work**. It reports end-window mock counters and `completion_claim:false`. Sampling can miss bursts. Harness CPU includes client, mock and sampler; sampler brackets separately report observer overhead without treating subtraction as exact target CPU correction. Compare identical dimensions, packages, binary profile, database seed and environment across repeated runs. Binary hash binds the executable artifact; source/build-profile attribution still requires caller evidence.

Pure tests are in `_tests/core.test.cjs`. Central QA command: `node --test scripts/node/ai-gateway-concurrency/resource-benchmark/_tests/core.test.cjs`. They reject PID reuse/regression/incomplete thread children, falsely summed PSS peaks, secret-bearing artifacts and partial/wrong/duplicated completed output. Implementation handoff only runs `node --check`; runtime and acceptance evidence belong to Root's central QA.

## Mixed upload evidence

Use the same required environment and command above with `RB_WORKLOAD=mixed RB_UPLOAD_BYTES=1048576`. Each batch starts `RB_CONCURRENCY` Responses requests and the same number of real `/api/console/files/upload` requests together, waits for every operation to settle, and fails on any error. Uploads omit `file_table_id` to exercise default selection, use deterministic binary content and `application/octet-stream`, then authenticate a GET to the returned table/record content route with the existing temporary owner. The oracle checks returned binding, record size/MIME/filename, HTTP content length/MIME, exact readback size and SHA-256. Reports retain only hashes, sizes, MIME and timings, never binary bodies or sessions.

1 MiB is the normal success fixture. The existing Console multipart endpoint has Axum's default 2 MiB body limit; `RB_UPLOAD_BYTES=8388608` is an explicit adverse fixture expected to fail with HTTP 413 under that contract. The harness does not bypass the limit, skip an upload, or count that failure as a successful performance sample. Initialization, permissions and isolation failures also fail the run. Use the 1 MiB fixture for successful comparative CI measurements; unchanged limit failures are boundary evidence.

Per-batch `uploads` records contain upload, readback and total latency, uploaded/read bytes and SHA-256; `upload_file_count` and `upload_bytes` describe that batch, including warmups separately. `objects_before`, each measured round's `objects_after`, and final `objects_after` count actual regular files under the isolated object directory: `bytes` is logical file length, `allocated_bytes` is filesystem blocks × 512, and `files` includes metadata sidecars and any runtime artifacts. These snapshots occur outside measured windows, before cleanup; they are not just the sum of requested upload sizes. Database snapshots retain their original scope. Harness CPU includes multipart construction and hashing as well as existing mock/sampler work. Mixed evidence covers text/upload coexistence; callbacks, images and other protocols require their existing separate conformance evidence.

Focused fixture checks: `node --test scripts/node/ai-gateway-concurrency/resource-benchmark/_tests/*.test.cjs` (run in the centralized QA batch).

## Root #2691 same-load comparison

Run this matrix only in fresh central QA after all product changes and fixtures are assembled and Root has frozen the candidate's exact 40-character source SHA. The W5 development input is not that frozen candidate. Baseline B0 is `8f03e003a192106566e2d115698fc4d4ab4d9c29`. Retain one already-built executable for B0 and one for the frozen candidate, each with source SHA, build profile/toolchain receipts and executable SHA-256 below `repo/tmp/test-governance/`. Use the same profile for both. Verify every report's `provenance.api_binary_sha256` against the corresponding saved binary; P5/P10 runs of one source must have the same executable hash. Build once per source; changing P does not require rebuilding.

Keep `RB_REPO_ROOT`, all three official package archives and their hashes, upload package budget, mock, seed procedure, history and upload bytes fixed. Each comparative run needs a fresh dedicated disposable DB initialized by the existing fixture and a fresh artifact directory; database lifecycle remains Root/caller-owned. Remove unintended ambient `API_DATABASE_*` overrides in the private launcher. For the four common cells, set only the P override and leave the M override unset: P is recognized by both B0 and candidate, whereas an explicit candidate M is a separately labeled experiment. Do not introduce D/I/T/full-budget defaults to make this matrix run.

Use these same nonsecret dimensions for every run:

```text
RB_WORKLOAD=mixed
RB_HISTORY_BYTES=8192
RB_UPLOAD_BYTES=1048576
RB_CONCURRENCY=3
RB_WARMUPS=1
RB_ROUNDS=3
RB_IDLE_MS=3000
RB_DRAIN_MS=3000
RB_SAMPLE_MS=100
```

For each P, execute **B0 → candidate → candidate → B0** in that order. This gives four logical cells and eight independent runs:

| Cell | Saved binary/source receipt | P override | ABBA artifact suffixes |
| --- | --- | --- | --- |
| B0-P5 | B0 SHA above | `RB_DATABASE_POOL_MAX_CONNECTIONS=5` | `p5/01-b0`, `p5/04-b0` |
| candidate-P5 | Root's frozen candidate SHA | `RB_DATABASE_POOL_MAX_CONNECTIONS=5` | `p5/02-candidate`, `p5/03-candidate` |
| B0-P10 | Same saved B0 binary | `RB_DATABASE_POOL_MAX_CONNECTIONS=10` | `p10/01-b0`, `p10/04-b0` |
| candidate-P10 | Same saved candidate binary | `RB_DATABASE_POOL_MAX_CONNECTIONS=10` | `p10/02-candidate`, `p10/03-candidate` |

For each row/position, the private launcher selects that `RB_API_BINARY`, its fresh `RB_DATABASE_URL`, and `RB_ARTIFACT_ROOT=repo/tmp/test-governance/gateway-2691-resource/<suffix>`, then calls the existing `node scripts/node/ai-gateway-concurrency/resource-benchmark/run.cjs`. Credentials remain in the private launcher environment. Mock traffic remains 256 deltas with a 10ms gap; no upstream/model call is introduced. The 1 MiB upload is the normal successful mixed fixture; the existing 8 MiB/413 case stays separate from this healthy-load matrix.

Each successful run plans 9 measured Responses requests and 9 upload/readbacks, plus 3 of each in its separate warmup. Thus each cell plans 18 measured samples of each kind across its two runs. Report actual `n_recorded` from retained round records, preserving every failed/interrupted run and its failed phase/diagnostic; planned n is not observed n. The runner retains the first sanitized failing operation, not a complete per-request error ledger for a failed batch. Count observed failed/interrupted runs and error/timeout diagnostics; mark unavailable aggregate per-request timeout/error totals as unknown rather than zero. Partial failed batches may have no retained measurement window and must not be silently treated as successful samples.

Keep CPU seconds, workload/drain boundaries, sampled RSS/PSS and observer overhead with their existing completeness/null indicators. The runner does not collect OOM counters; Root must retain available owned-process exit and host/cgroup OOM receipts separately, or mark that evidence unavailable. PG snapshots expose database/user-table heap/index/TOAST bytes, optional dedicated-cluster WAL, and optional host-visible DB process-tree CPU/memory. They do not measure connection admission, advisory waiters, callback progress or per-request PG cost.

Mechanism experiments use the real PG/Host starvation, cancellation, ownership and callback fixtures in Root's Test Batch. This runner measures healthy Responses SSE plus upload/readback coexistence. It does not replace callback/retry/protocol conformance tests or prove the starvation mechanism from healthy-load latency. Report the observations even when B0 and candidate are close; no performance improvement is assumed.

The optional `ai-gateway-concurrency.yml` manual entry with `resource_comparison=true` builds all three API sources using Cargo `--release`, records the profile/toolchain with each source SHA, and compares them using identical provider archives. `source_only=true` selects the separate source-acceptance batch instead; reusing an accepted product SHA does not rerun that batch during a profile-only resource measurement.

Before each source build, `cargo clean --release --workspace` removes that workspace's build artifacts while retaining third-party dependency caches. The compiler's JSON receipts must bind the affected local crates to that source path with `fresh=false`, including the optimized API binary. A source SHA plus an old executable digest is insufficient: shared-target builds across worktrees can otherwise reuse workspace artifacts when the next checkout has older mtimes. Keep each build's clean log, JSON receipts, source/profile/toolchain and copied executable digest together.
