# Native recovery acceptance

Run in the centralized QA batch against the frozen candidate API and OpenAI 0.2.71 package.

```bash
NRA_API_BINARY=/absolute/candidate/api-server \
NRA_PLUGIN_PACKAGE=/absolute/openai@0.2.71.1flowbasepkg \
NRA_ARTIFACT_DIR=/absolute/tmp/test-governance/native-recovery-fix-20261002/mock \
node scripts/node/ai-gateway-concurrency/native-recovery-acceptance/run.cjs
```

Uses an isolated database and random loopback listener. It never executes generated tools. The actual gateway and plugin must process a healthy custom-tool seed, lifecycle-only disconnect and rebuild, then 58 partial tool deltas followed by premature close. That stream must fail without a completed/tool-commit event. A new standard incremental Responses create using the accepted tool output must succeed, with exactly one persisted callback resume attempt. Logs retain model gpt-6-luna/max and actual binary hashes. All owned sessions/processes/databases/sockets are disposed in finally.

The original pre-fix negative is retained in the gateway main worktree at tmp/test-governance/native-recovery-409-20261002/reproduction. Long-task acceptance reuses responses-long-session-acceptance with RLS_MIN_SPAN_MS=3600000, RLS_SUBAGENT_MODEL=gpt-6.1-sol and RLS_ORDERED_STEER=true; compaction is assessed by successful protocol completion and continuation, without grading exact remembered strings.

## Client HTTP recovery after a WebSocket close

```bash
NRA_API_BINARY=/absolute/candidate/api-server \
NRA_PLUGIN_PACKAGE=/absolute/openai-0.2.74.1flowbasepkg \
NRA_WEB_REPO=/absolute/repository-with-installed-web-dependencies \
NRA_ARTIFACT_DIR=/absolute/tmp/test-governance/responses-1008/http-recovery \
node scripts/node/ai-gateway-concurrency/native-recovery-acceptance/http-recovery.cjs
```

This task-owned gateway uses inherited node transport and a WebSocket-default provider. The mock closes WebSocket requests with code 1008 before output and completes HTTP Responses through SSE. The provider must report a retryable physical interruption with no semantic commit. AI Native preserves its NativeOpaque replay fence and returns an honest failure; Codex owns the client retry. The client's full-input HTTP retry must complete once, both with an absent cursor and with `previous_response_id: null`. Assertions inspect the actual durable recovery receipt and compare actual upstream model, reasoning, tools, input and other native fields. The fixture preserves peer-reason redaction and cleans its own API, database, session and sockets. Existing `run.cjs` explicitly forces WebSocket so its HTTP-carried callback recovery remains a separate contract.
