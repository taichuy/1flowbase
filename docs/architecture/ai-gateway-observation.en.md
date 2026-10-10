# AI Gateway translation and execution evidence

[中文](ai-gateway-observation.md)

## Responsibilities

Execution follows `protocol mapping ⇄ AI Native ⇄ provider plugin ⇄ upstream`.

- Mapping parses client protocols and projects responses. Client traces retain actual requests and responses, not reconstructions from Native data.
- AI Native owns unified execution semantics and observes actual plugin inputs, results and errors. A snapshot proves what the plugin received, not the bytes sent upstream.
- Provider plugins adapt Native to upstream protocols and own upstream communication, not trace storage or queries.
- Logging uses the existing runtime event repository for facts and body locations. Node bodies and client bytes are archived separately. Coalescible text deltas remain live; complete items, tools, usage, errors and interruption evidence persist at semantic boundaries. Observation failures cannot change execution, client protocols, workflow state or billing settlement.

## Required records

Retain actual models, parameters, context, tool definitions and results, with run/node/invocation/attempt identity, errors and capture completeness. Internal calls have their own evidence even when clients cannot see them.

Do not store temporary translation objects or rerun translation/workflows while reading logs. Existing ledgers remain the usage and billing authority; provider usage in Native results is observed evidence, not a second settlement system.

## Reusing bodies within a call

Request/reply snapshots use unique UUID step keys. Tool results already contained in the request, or tool calls exactly equal to values in the reply, may use `body_ref = { step_key, pointer }` with JSON Pointer. Matching call IDs alone is insufficient. If a request is truncated or its snapshot is not admitted to the capture queue, retain independently capturable tool bodies.

For `body_format = native_reply_v2`, omit nested `result.final_content` only when it exactly equals outer `final_content`. Backend readers restore the existing detail structure; the frontend need not recognize storage formats. Preserve unknown extensions and distinct stream/terminal evidence.

Resolve references only within the same run, node, invocation and attempt, to single-version snapshots that do not themselves reference other records. Do not use mutable historical request/reply keys, cross-call semantic references or delta chains.

Lists read narrow indexes; details load snapshots on demand. Historical records without references retain their reader. Missing references/pointers, overwritten sources or reference chains fail explicitly: no reconstruction, cross-call reads or historical writeback. Facts share log retention and cleanup. Future single-event deletion must handle references before removing source bodies.

## Storage and recovery boundaries

Physical body reuse is distinct from semantic `body_ref` relationships. Native/provider observations can share immutable canonical content within the same application/scope. SHA-256 matches require full JSON equality checks. Raw Native model-call input strings use immutable manifests, items and ordered references: complete array items share exact bytes, literal spans retain whitespace, field order, number spelling and escaping, and repeated occurrences remain distinct. Reads verify ownership, references, byte length and whole-string SHA-256 before restoring the original string. Unknown or non-losslessly separable shapes retain canonical storage. Every observation retains its identity, source and format. Equal bodies do not imply equal calls, execution idempotency or original client bytes; there is no cross-tenant or similarity-based reuse.

Canonical content first created by observation archival carries explicit ownership. After the last durable reference is removed, deferred transactional triggers check runtime events, client sections, context, recovery and legacy shadow references before reclamation. Reused pre-existing canonical content retains its original lifecycle. Native manifests are reclaimed after their last event reference, and items after their last manifest reference. Shared writes and reclamation use the same application lock. Run deletion leaves no newly orphaned observation bodies; selective backups include all new tables by application owner.

Node catalogs retain identity and execution state. New client Step/Section writes use occurrence catalogs rather than one runtime event each; Integrity, NodeLink and ResponseLink remain events. The durable sequence high-water mark covers events and catalogs without renumbering old numeric cursors. Sections reuse exact canonical values within application/scope. Parameters/results use locators only when exactly equal to the designated child in a restored overview. Ordinary timing reuses occurrence `observed_at`; special shapes retain full values. Backend Rust selects children after restoring originals; DTOs remain unchanged.

Client requests retain actual bytes and versioned compact frame catalogs separately; nodes only link requests. Stream frames no longer each require an event row. New parts use lossless zlib compression with length, directory and SHA-256 validation. Readers decode parts and paginate original frames, retaining exact bytes, timestamps, kinds, ordering and sequence. The v0 reader supports old wire/legacy formats; SQL legacy references keep their original parts. Backpressure, head locks and post-commit completion receipts ensure accepted bytes persist.

Historical maintenance requires an explicit run allowlist. It switches references only after real-reader restoration and full value/frame verification, and is reentrant. Startup/request paths do not migrate history. Event identities, cursors and Step revisions retain their envelopes; verified Section bodies may become small references. Unsafe historical Sections, including NUL cases, retain the old body/reader and record a skip so later batches continue. New data remains lossless; long-term context is not copied into ephemeral storage.

Pre-binding capture closes only after final-owner EOF and accepted-frame drain. Without a run owner, the repository explicitly cleans up heads/parts. Cleanup failure returns a completion error and retains bytes; late binding during drain preserves the trace. TTL does not infer execution state. A crash before completion can still leave records requiring investigation. Removing a UI tab does not stop capture; display policy and evidence retention are separate.

## Client requests and workflow internal events

The UI distinguishes client requests (original mapped protocol) from workflow internal events (actual inputs, context, tools, configuration and outputs by LLM call/attempt). Purpose comes from actual `generate:false`, an admitted tool resume or the real operation, not empty responses, historical tool messages or handshake labels.

Each HTTP/WS capture ID reaches the current execution segment through a Host-private, consume-once `WorkflowObservationContext`. It is not deserialized from public JSON and does not enter plugin requests, user variables or checkpoints. Native events carry the current `trigger_request_id` and separate `context_flow_run_id`/`context_response_id`; prior sources require existing authorization and response-context resolution.

Both captures may arrive asynchronously. Native narrow indexes retain explicit identities. Readers resolve triggers in the same run and emitted context responses within the same application and explicitly identified prior run. Do not add ordering-dependent foreign keys, guess by node/time or backfill history. Missing sources produce no navigable link.

Both lists support request filtering and first-page focus, then ordinary cursor pagination. Detail links open the exact request, and client requests link to their calls with explicit one-to-many relationships. Existing console authorization applies, including application checks on cross-run targets. The frontend preserves filters, selection and scroll when returning. Existing node trees, input/processing/output views and Resume timelines keep their responsibilities.

## AI Native traces and protocol diagnostics


Gateway mapping adapts client protocols to AI Native; provider plugins translate Native to upstream protocols and own that upstream communication. Logging does not move translation ownership.

The main trace records safe Native input, standard output/tool requests, outcomes and provider extensions. Tool requests and submitted tool results do not prove execution; actual workflow execution supplies that evidence. Optional raw provider protocol capture is separate from the main trace. Native snapshots must not be labeled actual upstream bytes, and unknown provider metadata does not prove internal retries or success.

`FLOWBASE_PROVIDER_PROTOCOL_CAPTURE=1` enables raw capture explicitly on the API server after restart; default is off. Only calls with a raw observation sink negotiate this capability with the plugin. Client input cannot enable it. Unsupported optional capture does not disable Native traces.

Provider stream timing normally retains exact per-attempt summaries—event counts, total bytes, counts by kind, inbound time range and maximum write latency—not per-event arrays. Set `FLOWBASE_PROVIDER_STREAM_TIMING_CAPTURE=1` server-side and restart to retain the full five-field detail. The mode freezes at call start and does not truncate records. Summary lives in `metrics.attempts[].provider_stream_timing_summary`; details use sibling `provider_stream_timing`. This changes diagnostic detail only, not semantic, tool, recovery or delivery facts.

Semantic and raw capture completeness are independent: a failed call may still have a complete trace. Missing raw capture does not downgrade complete Native evidence. Dropped observations, write errors, cancellation and limits must be represented honestly. Lists read summaries; step selection loads `view=semantic`, and explicit protocol inspection loads `view=protocol`. Raw evidence links by invocation/attempt, without inventing a one-to-one step/network relationship. Historical projections retain `supplier_protocol`, new Native records use `ai_native`; do not fabricate Native history from old protocol bytes or rebuild projections during GET.

## Log read models and observed-output settlement

Settling a log projection does not mean the workflow or billing has completed. A `waiting_callback` episode has a five-minute projection deadline. Actual progress—entering the wait, changes to member runs or invocation/callback counts—starts another episode. Repeated projector writes, usage corrections and page reads cannot extend it. This deadline only governs the read model; it neither cancels execution, fabricates callbacks nor removes recovery evidence.

Settled projections prefer the last client text, then a non-conflicting complete provider assistant message, then existing `final_output`. Real text produces `observed_output`; absent text produces a `timeout` source and `Timeout` placeholder. Backend overview includes business answers sourced as `persisted_answer` or `provider_output_item` in assistant messages. Timeout placeholders remain separate projection output; neither fragments nor placeholders become real replies. Overview assigns unique sequence positions to the returned list, avoiding collisions between run-local sequences.

Costs use durable snapshots for completed members when available, otherwise existing cost-ledger facts. Usage aggregates existing usage-ledger rows, falling back to member summaries only when ledger rows are absent. Recompute from facts rather than adding to prior projections; missing observations remain unknown, never invented zeros. Only the log projection changes: execution state, callbacks, usage and cost ledgers remain untouched. This is not a second billing authority.

Implementation: [projection migration](../../api/crates/storage/durable/postgres/migrations/20261010120000_observed_log_projection_settlement.sql), [overview reader](../../api/crates/storage/durable/postgres/src/orchestration_runtime_repository/agent_logs/reads.rs).

Task aggregation and individual attempts retain separate states. Any active member keeps the task outcome `in_progress`, even when an earlier answer exists. Once all members are terminal, task status prefers the latest generate attempt, falling back to the latest other call if no generation exists; counting or compaction cannot mask generation outcomes. Failed attempts remain in source runs and traces. Known costs include all attempts, with all-unknown totals remaining NULL; a successful retry does not erase earlier calls.

Implementation: [terminal task projection](../../api/crates/storage/durable/postgres/migrations/20261010130000_project_terminal_task_attempt_outcome.sql).

## Task boundaries in conversation detail

Conversation detail obtains `log_conversation_id` independently from the run overview rather than the current list page. `around_run_id` is an inclusive task cutoff for the initial page and both paging directions: a completed task ends at its final response. Omitting the anchor still opens the latest conversation page. Continuation requests resolve to their owning task; a prewarm anchor locates the preceding business turns.

Explicit `request_kind=prewarm` denotes protocol connection setup. Preserve its raw request and conversation membership, but exclude it from business turns and their pagination. Empty input, missing output, or a missing `turn_id` cannot establish prewarm classification. System context accompanies its business turn; later prewarm context must not follow the selected task.
