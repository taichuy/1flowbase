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
