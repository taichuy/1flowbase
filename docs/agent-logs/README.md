# Agent Logs source collector

This repository includes a runnable **source CLI**, not a published npm package. Use Node.js 20 or newer and a checkout containing `scripts/node/agent-logs-collector.js` and its sibling directory. No dependency installation is required. A future distributable package must be released separately; do not use an invented `npx` package name.

Create an Agent Logs application, obtain its application API key, and set `FLOWBASE_AGENT_LOGS_API_KEY` through your shell or secret manager. The key is not accepted as a command argument and is not printed. Select a local source explicitly; the collector does not search your home directory.

```bash
node scripts/node/agent-logs-collector.js import \
  --endpoint https://your-host/api/logs/v1/events \
  --source /your/chosen/codex-directory \
  --state /your/private/agent-logs-state.json

node scripts/node/agent-logs-collector.js watch \
  --endpoint https://your-host/api/logs/v1/events \
  --source /your/chosen/codex-directory \
  --state /your/private/agent-logs-state.json
```

Choose the Codex root containing both `sessions` and `archived_sessions` to include historical and archived rollouts. A single `.jsonl` file is also supported. All selected JSONL files are scanned recursively; symlinks are skipped to avoid escaping the selected directory or following cycles. There is no total event/file cap. `--batch-size` sets the transport batch size (default 100); `--interval-ms` sets watch polling (default 2000). `watch` retries failed collection on subsequent polls. `import` exits unsuccessfully on an upload/parse/checkpoint error. `--help` lists all options.

The endpoint is the complete ingest URL. HTTP is available for local development; use HTTPS when transmitting application credentials over a network. Redirects are refused. The state stores source identity, endpoint, source-client identity and acknowledged offsets, without the API key or conversation text. Protect the source files and state as local private data. Preserve the state and generated `source_id` for resumptions. `--source-id` provides an explicit stable installation identity and must agree with existing state. Use a separate state for a different endpoint or adapter. Run only one collector per state; after an abnormal process termination, verify that no collector is active before removing the adjacent `.lock` file.

## Durability and source meaning

Only newline-terminated JSONL records are read. A partial trailing line stays uncommitted and will be read after completion. Batches advance the checkpoint only after a complete successful durable receipt. Network failures, rejected requests and incomplete receipts retain the previous checkpoint. Crash between server commit and local checkpoint persistence causes a replay; stable IDs allow the server to return duplicates. An acknowledged prefix that is modified or truncated is rejected. Re-reading is intentional: source context is reconstructed deterministically before resuming.

The immutable first rollout header and byte position identify events. Moving the same rollout into `archived_sessions` preserves its identity. Two identical texts at distinct positions are distinct facts; the collector does not deduplicate by content. Event timestamps come from the source, never the collector clock. Ordinals, `history_base` rollout-prefix pointers, parent thread and root session facts are preserved in `raw`; they are not confused with local byte offsets or child thread IDs.

Only source-declared turn IDs establish a task: `turn_context`, task/turn-start events, typed usage or response-item passthrough metadata. Initial session-level and pre-turn facts wait for the first reliable turn, then use that turn's ownership with unchanged event identities. A file without a reliable turn is reported as awaiting source turn identity; it produces no fabricated user task and does not advance its checkpoint. Prefix references are retained as facts; this collector does not synthesize child events by copying the referenced parent rollout. Inlined inherited history is marked from source metadata/ordinal boundaries and remains excluded from new task input and usage by the backend.

Codex `response_item` user/assistant messages provide conversation facts. Only explicit `phase: "final_answer"` declares final output. Commentary, tools, compaction, context, unknown record types and raw source facts remain in the trajectory. Presentation `event_msg` user/agent mirrors remain context observations, avoiding duplicate conversation projections. A completion event records task end and never manufactures final text from `last_agent_message`. Codex `turn_aborted` is normalized to `kind: "task_end", phase: "cancelled"`, preserving the complete raw record; the backend consumes this source-neutral cancellation fact. Older files without explicit final phase preserve the unknown phase; no model/phase/provider is guessed. `model` and `model_provider` are copied when explicitly supplied, without changing provider identifiers.

`token_usage_record.usage` is a response delta with `response_id`; `turn_token_usage` and `thread_token_usage` remain raw cumulative observations. Legacy `event_msg.token_count.info.total_token_usage` is sent with `basis: "cumulative"`; `last_token_usage` is retained raw. Historical cumulative snapshots without a reliable response ID are observations of session/thread history, not attributable task/response consumption. They remain in the trajectory and raw source evidence and must not be added to task tokens or converted into guessed per-turn increments. Cumulative snapshots must not be summed with response deltas; a reliably response-attributed cumulative snapshot can supply that response only when it has no deltas. Pricing, billing and usage aggregation belong to the backend; this CLI does not calculate charges.

## API envelope and receipt

```json
{
  "schema_version": "1flowbase.agent-logs/v1",
  "source_id": "stable-installation-id",
  "source_client": "codex",
  "events": [{
    "event_id": "stable-byte-position-id",
    "source_session_id": "thread-id",
    "source_task_id": "turn-id",
    "parent_source_task_id": null,
    "sequence": 1,
    "occurred_at": "2026-10-07T08:00:00Z",
    "kind": "assistant",
    "content": "Final answer",
    "phase": "final_answer",
    "name": null,
    "call_id": null,
    "model_id": null,
    "provider_code": null,
    "usage": null,
    "inherited": false,
    "raw": {}
  }]
}
```

Send `POST /api/logs/v1/events` with `Authorization: Bearer <application-key>`. The event kinds are `system`, `user`, `assistant`, `tool_call`, `tool_result`, `usage`, `context`, and `task_end`. Usage contains `basis: "delta" | "cumulative"` and optional `response_id`, `input_tokens`, `output_tokens`, `input_cache_hit_tokens`, `cache_write_tokens`, `total_tokens`.

The canonical HTTP success body is `ApiSuccess<AgentLogsReceipt>`: `{"data":{"accepted_events":1,"duplicate_events":0,"record_ids":["record-id"]},"meta":null}`. The collector reads the receipt only from `JSON.data`; unwrapped top-level fields are not accepted. The receipt inside `data` contains `accepted_events`, `duplicate_events`, and string-array `record_ids`. Accepted plus duplicate counts must equal the complete batch size. The server commits the entire batch or rejects it; conflicting payloads under an existing identity are errors. An application API key authorizes ingestion, not model generation.

## Local adapter boundary

`--adapter /absolute/path/to/installed-adapter.cjs` selects a trusted installed CommonJS script. Custom adapters execute local code with the collector's privileges; select scripts you trust. The single loader requires this interface:

```js
module.exports = {
  sourceClient: 'my-client',
  createContext(firstLine) { return {}; },
  convert(line, context, position) {
    // Return a normalized envelope event without event_id/sequence, or null.
    // position.start/end are byte offsets. Do not perform HTTP/checkpoint writes.
    // source_task_id=null postpones ownership until a reliable turn arrives.
  }
};
```

The shared scheduler supplies stable event identity and sequence, uploads and checkpoints. Adapters only convert format facts and maintain deterministic parsing context. This is a local source adapter boundary, not a server plugin/runtime slot. The bundled Codex adapter was checked against the locally supplied Codex Rust protocol/history definitions; source-version drift and absent metadata remain explicit limitations.

Behavior fixtures (run by the centralized QA batch):

```bash
node --test scripts/node/agent-logs-collector/_tests/collector.test.js
```

See [中文说明](README.zh-CN.md).
