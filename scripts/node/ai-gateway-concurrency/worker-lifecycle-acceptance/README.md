# Real Codex worker lifecycle long-task fixture (Issue 2153)

Run only in the frozen assembly's centralized QA batch. This fixture does not start or stop an API server. It launches its own installed Codex **0.155.1 app-server stdio** with a private `CODEX_HOME`, real `gpt-6-luna` / `max`, zero configured provider retries, and read-only sandbox. Workspace: `/home/taichuy/git/1flowbase_latest`. No Codex installation/build or user config edits.

Root must supply the deployed candidate's exact live identity. Parent and worker arguments are **whole argv tokens**, not regexes or substrings. Use an actual distinctive candidate argument (parent executable token is acceptable if it uniquely identifies the authorized candidate); verify that parent PID belongs to the isolated 7600 API. Worker executable is the installed version's real path, digest is its SHA256. The worker must be a direct child of that parent. Discovery fails honestly if this topology is unavailable or ambiguous. Baseline 7800 must remain untouched.

```bash
WL_KEY_FILE=/private/path/gateway-key \
WL_PARENT_PID=<candidate-api-pid> \
WL_PARENT_EXE=/absolute/candidate/api-server \
WL_PARENT_ARG=<exact-candidate-argv-token> \
WL_WORKER_EXE=/absolute/installed/openai/version/bin/openai-provider \
WL_WORKER_ARG=<exact-supplier-argv-token> \
WL_WORKER_SHA256=<installed-supplier-sha256> \
WL_CANDIDATE_SHA=<frozen-assembly-sha> \
WL_MODEL_CATALOG=/private/path/client-model-catalog.json \
node scripts/node/ai-gateway-concurrency/worker-lifecycle-acceptance/run.cjs
```

Optional `WL_CODEX` selects the existing installed binary, `WL_BASE_URL` defaults to and only accepts `http://127.0.0.1:7600/v1`, and `WL_ARTIFACT_DIR` must end in `tmp/test-governance/2153/long-task`. Keep credentials in a file, never command-line arguments. Start with an unused artifact directory; archive prior runs outside the next run's directory. Artifact directory permissions are private and the credential remains in the child environment, not config or artifacts. Timeline applies exact credential and bearer redaction. Do not supply secrets in source audit files.

The generated schema bundle captures installed protocol provenance. `timeline.jsonl` preserves redacted RPC requests, actual notifications, client stderr/exit, PID/executable/start-time proofs, RSS/PSS/FD snapshots and monotonic times. `manifest.json`, `progress.json`, and `verdict.json` preserve source SHA, workspace, model, identity, cold-start timings and result. Child thread history is persisted in the private home; only the fixture's own client process is reaped on exit.

AC evidence: actual spawn/wait result items; explicit compact RPC plus completed `contextCompaction`; sentinel read by a successful tool before/after compaction and reproduced in the same persistent thread; active-turn steer acknowledged and unique nonce in that turn's final message; verified SIGKILL between turns and successor PID; semantic/reasoning delta before midcall verified SIGKILL, exactly one failed terminal, no retry notification, then one lawful new successful invocation and new PID. A midcall kill that misses the live upstream call or transparently recovers fails the run; it is not blindly retried. Each successful work turn requires real successful shell-tool evidence and substantial analysis. The source inventory advances through six audit dimensions with no sleeps or filler, and the total monotonic duration is at least 5400 seconds. Useful completed turns cannot have gaps over 15 minutes.

Failures/timeouts are retained with a nonzero verdict; absent protocol events are never inferred. Long duration is mandatory, with no short-run acceptance flag. The fixture's semantic-output trigger can expose a race where the supplier has already finished a response; this produces a failed acceptance verdict and requires a separately reviewed new run, not hidden retries.

Central QA unit command (does not launch clients/servers):

```bash
node --test scripts/node/ai-gateway-concurrency/worker-lifecycle-acceptance/_tests/*.test.cjs
```

Counterexamples reject missing subagents, compaction, steer nonce, insufficient duration, missing successful tools, continuity gaps, duplicate failed terminals, retry notifications, unchanged generation and absent semantic output. Kill guards reject unrelated exe/PPID/digest, reused PID/start time and baseline-port arguments. These are fixture-oracle tests; they do not prove the long run or candidate runtime behavior.

Installed 0.155.1 rejects an unregistered `gpt-6-sol` subagent before calling the gateway. Supply a private `ModelsResponse` catalog through the existing `model_catalog_json` configuration. This run registers the requested slug using the installed-client-compatible `gpt-5.6-sol` metadata template from the pinned local Codex source; this is client tooling metadata, not a claim about model capabilities or a gateway business/resource limit. The wire request remains `gpt-6-sol`; no model alias or substitution is applied in the gateway. Catalog SHA256 is captured and `model/list` must advertise that exact slug. Completed actual spawn events must report `gpt-6-sol/medium`; an unknown or substituted model fails early and the acceptance oracle rejects it. Only the fixture private configuration is edited.
