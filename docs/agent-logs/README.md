# Agent Logs native collector

[中文](README.zh-CN.md)

The collector has two installation stages. First, install the signed distribution package from the remote extension catalog **in 1flowbase**. Then open its **Download collector CLI** detail and run the command on your computer. The installer, native archive, checksums and documentation are served by your own 1flowbase at version-pinned URLs. They remain available if the remote repository is offline. “Installed in platform” describes the retained distribution package; it does not indicate that a client process is online.

On an Agent Logs application's **Collector CLI** page, select the Codex tab to open its details; **All** returns to the directory. Paste an API key for this application, or select **Generate key** to create a real application key and fill the field. Copy the Shell or PowerShell command and run it on the computer running Codex. If the field is empty, the installer still prompts for the key locally. The official Rust executable does not require Node.js, a Rust toolchain or a repository checkout.

The installer downloads a checksummed native release, saves private configuration and starts a user background service. Linux uses user systemd, macOS uses LaunchAgent and Windows uses a user scheduled task. It resumes when the same user logs in after a reboot. If a supported service is unavailable, installation reports that explicitly; `--no-start` allows manual process supervision.

Each application uses its own installation ID and checkpoint. The default Codex root is installation-time `CODEX_HOME` or `~/.codex`; only `sessions` and `archived_sessions` are read, including existing history and future complete records. Custom source paths are supported. Installation and collection do not modify Codex source logs.

The input retains the key only in the current detail view and clears it on reload or navigation away. It does not write to browser storage or query caches. The command preview masks the key; the copied command passes its real value to the local installer through the `FLOWBASE_AGENT_LOGS_API_KEY` environment variable, never through download URLs or installer CLI arguments. The installer saves it in private `config.json` on your device. Generated keys can be managed or revoked on the application API page. Upload sends it only to the specified 1flowbase endpoint as a Bearer header. The endpoint is the exact `/api/logs/v1/events` URL; use HTTPS across a network. Redirects are refused.

[Official installation, upgrade and uninstall instructions](https://github.com/taichuy/1flowbase-official-plugins/blob/main/runtime-extensions/@taichuy/codex-logs-collector/README.en.md)

Native commands are `codex-logs-collector import --config PATH` and `codex-logs-collector watch --config PATH`. Retain `state.json` and its generated source identity across reinstalls. A checkpoint has one process owner; OS advisory locks are released automatically after a crash. Reconfiguration can rotate the key, while changing endpoint/source requires a separate installation.

## Durability and source meaning

Only newline-terminated JSONL records are read. A partial trailing line stays uncommitted and will be read after completion. Batches advance the checkpoint only after a complete successful durable receipt. Network failures, rejected requests and incomplete receipts retain the previous checkpoint. Crash between server commit and local checkpoint persistence causes a replay; stable IDs allow the server to return duplicates. An acknowledged prefix that is modified or truncated is rejected. Re-reading is intentional: source context is reconstructed deterministically before resuming.

The immutable first rollout header and byte position identify events. Moving the same rollout into `archived_sessions` preserves its identity. Two identical texts at distinct positions are distinct facts; the collector does not deduplicate by content. Event timestamps come from the source, never the collector clock. Ordinals, `history_base` rollout-prefix pointers, parent thread and root session facts are preserved in `raw`; they are not confused with local byte offsets or child thread IDs.

Only source-declared turn IDs establish a task: `turn_context`, task/turn-start events, typed usage or response-item passthrough metadata. Initial session-level and pre-turn facts wait for the first reliable turn, then use that turn's ownership with unchanged event identities. A file without a reliable turn is reported as awaiting source turn identity; it produces no fabricated user task and does not advance its checkpoint. Prefix references are retained as facts; this collector does not synthesize child events by copying the referenced parent rollout. Inlined inherited history is marked from source metadata/ordinal boundaries and remains excluded from new task input and usage by the backend.

Codex `response_item` user/assistant messages provide conversation facts. An assistant message explicitly marked `phase: "final_answer"` declares final output. Commentary, tools, compaction, context, unknown record types and raw source facts remain in the trajectory. Presentation `event_msg` user/agent mirrors remain context observations, avoiding duplicate conversation projections. Codex `task_complete` (wire alias `turn_complete`) explicitly declares completion text through a nonempty string `last_agent_message`; the adapter maps that declaration to `kind: "task_end", phase: "final_answer", content: <source text>`, keeping real turn ownership and the complete raw record. A missing, empty or whitespace-only completion text remains `content: null, phase: null`. This does not generate summaries or infer finality from ordinary assistant ordering. When an assistant-final and a completion-final overlap, the adapter preserves both facts; the generic backend projects exactly one conversation final and retains both trajectory originals. Codex `turn_aborted` is normalized to `kind: "task_end", phase: "cancelled"`, preserving the complete raw record; the backend consumes this source-neutral cancellation fact. Older assistant messages without explicit final phase preserve the unknown phase; an explicit completion-text declaration can still supply the final answer; no model/phase/provider is guessed. `model` and `model_provider` are copied when explicitly supplied, without changing provider identifiers.

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

Send `POST /api/logs/v1/events` with `Authorization: Bearer <application-key>`. `task_end` with `phase: "final_answer"` and nonempty content declares source completion final text; `phase: "cancelled"` declares cancellation without final content. The event kinds are `system`, `user`, `assistant`, `tool_call`, `tool_result`, `usage`, `context`, and `task_end`. Usage contains `basis: "delta" | "cumulative"` and optional `response_id`, `input_tokens`, `output_tokens`, `input_cache_hit_tokens`, `cache_write_tokens`, `total_tokens`.

The canonical HTTP success body is `ApiSuccess<AgentLogsReceipt>`: `{"data":{"accepted_events":1,"duplicate_events":0,"record_ids":["00000000-0000-0000-0000-000000000001"]},"meta":null}`. The collector reads the receipt only from `JSON.data`; unwrapped top-level fields are not accepted. The receipt inside `data` contains `accepted_events`, `duplicate_events`, and string-array `record_ids`. Accepted plus duplicate counts must equal the complete batch size. The server commits the entire batch or rejects it; conflicting payloads under an existing identity are errors. An application API key authorizes ingestion, not model generation.

## Local adapter boundary

The official shared Rust `agent-logs-collector` SDK owns source scanning, stable event identity, HTTP upload, full ACK validation, exclusive checkpoint ownership and durable persistence. Codex implements the SDK's source adapter interface and only converts its private format. It uses canonical Rust DTOs from a pinned main-repository revision, rather than duplicating protocol fields.

The old repository Node collector remains a development fixture oracle and is not the user installation entry. Native source and recovery tests run in the official plugin repository with `cargo test --locked --manifest-path sdk/agent-logs-collector/Cargo.toml` and `cargo test --locked --manifest-path runtime-extensions/@taichuy/codex-logs-collector/Cargo.toml`. Installation fixtures use synthetic logs and a local mock HTTP endpoint.


## Imported cost estimates

Imported costs match existing pricing rules by the exact `model_id` only. `provider_code` remains source metadata and does not filter prices. Rules must be enabled and valid at the event time, including their local time windows. If several rules match, the first eligible rule in the pricing list's stable order wins: provider code ascending, priority descending, effective start descending, then rule ID ascending. Missing prices use `zero/any`; missing usage is not fabricated. Estimates never debit balances. Native model invocations retain their separate provider-based billing behavior.

Price configuration changes do not automatically rewrite historical estimates. A database maintainer can explicitly run `agent_logs_reprice --application-id UUID --scope-id UUID`, supplying `API_DATABASE_URL` through a private environment variable. The command reads minimal persisted usage facts in record-ID order and updates costs only. It preserves messages, trajectories, event identities, bodies, tokens, and collector checkpoints, without re-uploading logs. Each record commits atomically; failed runs can be safely repeated.
