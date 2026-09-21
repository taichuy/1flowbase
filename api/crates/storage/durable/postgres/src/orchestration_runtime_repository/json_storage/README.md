# Runtime JSON Storage

PostgreSQL JSONB/TEXT cannot hold U+0000, while JSON protocol payloads can.
The adapter stores a query projection in the existing column and, only for
values requiring encoding, the exact serialized JSON in `raw_json_payloads`.
The sidecar maps column names to JSON serialization strings. It is a separate
schema column, never an object marker recognized in user input.

## Write Invariant

`lossless_json_parameter` produces an adapter-owned two-element parameter:
projection and optional original serialization. SQL assigns element 0 to the
JSONB column and element 1 to its sidecar slot in the same statement. TEXT uses
`lossless_text_parameter` and `->> 0`. SQL NULL remains SQL NULL; JSON null is a
non-null parameter whose projection is JSON null.

Updates remove affected old slots before merging new non-null originals.
Conditional assignments and COALESCE mirror the business-column condition in
the sidecar expression. Unchanged fields retain their slots. Batch events bind
separate projection/original columns inside the existing transaction.

If an object contains NUL in a key, its projected keys also escape existing
backslashes, so two distinct original keys cannot overwrite each other. The
query representation is not a source for recovery or exact-content equality.

## Read Invariant

`runtime_original_json` returns PostgreSQL **json**, not jsonb. That wire format
keeps the encoded escape until SQLx/serde_json decodes it in Rust. Do not cast
its result back to JSONB or apply PostgreSQL JSON operators/array extraction to
it: even `->` reparses strings and rejects NUL. Return the entire value and
select child fields in Rust after decoding. Callback privacy projections and
stream tool-call selection follow this rule.
TEXT reads select the serialized original under `<column>_original` and use
`original_optional_text` / `original_required_text` at the Rust row boundary.
Ordinary rows without original slots retain their previous values.

## Bounded Inventory

| Table | Encoded columns |
| --- | --- |
| flow_runs | input_payload, output_payload, error_payload, log_context |
| node_runs | input_payload, output_payload, error_payload, metrics_payload, debug_payload |
| flow_run_callback_resume_attempts | response_payload, error_payload |
| flow_run_callback_tasks | request_payload, response_payload, external_ref_payload |
| flow_run_checkpoints | locator_payload, variable_snapshot, external_ref_payload |
| flow_run_resume_claims | request_payload, error_payload |
| flow_run_tool_callback_inbox | result_payload |
| flow_run_events | payload |
| runtime_events | payload |
| runtime_canonical_contents | content |
| application_conversation_messages | content (TEXT) |
| application_run_conversation_message_items | content/query/answer (TEXT), native_message |
| application_run_trace_nodes | metrics_payload, content_ref (TEXT) |
| application_run_trace_node_contents | payload, source_refs |

Lifecycle writes and returning reads are in `flow_run_methods`, `artifact_methods`,
`waiting_state_methods`, `event_methods`, callback/claim/inbox owners, and
`storage_foundation_methods`. `detail_queries`, `read_methods`, `delivery_methods`
and lineage reads restore originals. Canonical deduplication and callback replay
compare original slots as well as projections. Checkpoint shadow fallback checks
both representations before choosing a canonical row.

Conversation writers/readers live in `application_run_log_methods`,
`application_run_logs`, and the public conversation repository implementation.
Trace payloads are owned by `application_run_trace_projection_methods`.

Native log facts preserve the existing `(source_key, run_id, sequence)` winner
and `(sequence, source_key)` output order. Exact JSON equality is evaluated in
Rust after restoring events and log-context tool results, with no added limit.
Tool inbox results are restored before assembling the callback response.

SQL filters on status, identities, well-known structural fields, logging cards,
summary text, trace navigation, and display search continue using projections.
Public conversation history, assistant native messages, trace content, checkpoint
recovery and callback idempotency use originals. Conversation titles display NUL
as U+2400; they never supply provider prompt content.

This is not a codec for arbitrary product configuration tables, identifiers,
provider metrics ledgers, or unrelated metadata.

## Verification Entry

`runtime_json_storage_tests` contains five real repository lifecycle tests using
isolated schemas and the formal migrations. `_tests/codec.rs` covers marker
collision, object-key collision, literal escapes and SQL/JSON-null distinction.
The source does not claim that these tests have been executed.
