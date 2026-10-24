# Responses compatibility across protocols

The gateway separates protocol mapping, AI Native semantics, and provider adaptation. Compatibility preserves representable semantics; it does not imply that every provider supports every Responses feature.

## Notifications and tool results

A `function_call_output` with an absent or null `call_id` is standalone external context. The mapping layer preserves its JSON output, name, namespace, and extensions. AI Native stores typed external context and uses a shared projection into an attributed user-level data message. It neither elevates the content to system/developer instructions nor fabricates a tool call ID. Attribution and content remain external data, not trusted model instructions.

Only nonempty call IDs participate in tool-result correlation. Empty or malformed IDs are rejected. Both full-history and `previous_response_id` continuations preserve notification/result order and continue to verify scope, complete result sets, duplicates, and history proofs. A notification cannot complete or reopen a tool task.


Chat-style providers require a complete, contiguous group of tool results immediately after the assistant tool calls. The shared projection places notifications arriving within that group in an external-context message after the results, with a timeline recording their original order and references to real results. Canonical history, result contents, and call IDs are unchanged. This projection does not repair incomplete or invalid result correlation.

## Transport selection and response style

Requests containing notifications or only `text.verbosity` retain both a semantic representation and the original Responses body. A selected provider supporting both `responses.native_passthrough` and `responses.native_output.v1` receives the original body; other providers use the semantic representation. Required native fields and existing provider continuation affinity cannot fall back. Recovery, output, and history metadata follow the transport actually selected.

On the semantic path, `text.verbosity` values low, medium, and high become a request-owned response style prompt, recorded as Normalized in the translation report. Model adherence is best effort and is not equivalent to a native provider parameter. Unknown text extensions and `text.format` still require native capabilities; invalid verbosity values are rejected.

## Provider responsibilities

The DeepSeek plugin lowers Native tool choices into Chat Completions: auto, none, and required become strings; named choices become function objects. The host owns notification projection, subprocess communication, and continuation state. Plugins do not need their own Responses emulation.

This path supports ordinary text, standard function tools, and the notifications described above. It is not complete Codex compatibility: custom grammar tools, strict tool schemas, hosted capabilities, and provider-private reasoning remain subject to their capability checks. Silently dropping fields is not a substitute for support.

## Recovery persistence and request admission

Protocol adapters own HTTP / WebSocket request admission. `PgProviderProtocolCapsuleStore` persists complete protocol contexts and continuation native histories without a smaller guessed business payload cap. The independent 2 MiB rejection has been removed; payloads above that former threshold remain encrypted, restart-readable and atomically consumable. Physical representation and actual database / I/O failures still propagate; histories are neither truncated nor silently discarded.

AEAD binding to flow run / capsule kind / slot key, digest / size verification, existing expiry and invocation authorization remain in place. Capacity and lifetime are independent. PostgreSQL regressions restore both large payload types exactly and verify ciphertext, locators and single consumption.

## Request and handoff size

Gateway compatibility endpoints, Native HTTP (including file inputs), and WebSocket do not impose built-in body, message, or frame byte caps. Host and shared SDK stdio preserve complete NDJSON messages. Write and acknowledgment windows schedule transport: an oversized message can occupy a window exclusively and make progress; accumulated responses and returned bodies have no transport quota. Synchronous provider callbacks cannot wait for a writer on the same executor, so accepted events remain queued until written. This does not imply constant memory use for arbitrarily large requests. Identity, cancellation, protocol validation, and actual I/O failures remain with their existing owners.

The host runtime event stream uses its existing required-delivery classification. Business deltas, output items, and terminal events are neither rejected by diagnostic payload or retention budgets nor evicted to make room for diagnostics. Live consumers retain backpressure; completed generations with confirmed durable persistence retain the existing approximately five-minute reclamation window. Diagnostic retention and inspection budgets remain separate. Durable history and its database source of truth are unchanged.

## Content projection and history ownership

The Chat ingress validates all messages before producing translation receipts and Native content in the existing message order. Each message is projected once; the latest user's already constructed content supplies the query and its media history. Assistant reasoning precedes visible content, and the current owner moves existing media blocks into the final history object instead of retrieving and copying them through JSON. Validation and projection retain their separate responsibilities for error precedence and content conversion.

Anthropic content-block validators own structural checks and supported-content decisions. Once validation succeeds, the query/history mappers construct the required text; validation does not create a text copy that is immediately discarded. Error precedence and ordered translation receipts remain unchanged. Anonymous unknown-field paths represent only the parent path and field count, without reading or sorting field names. Root-field processing still sorts names where they determine retention or removal, preserving the association between anonymous ordinals and decisions.

The Control Plane's `run_service/conversation_history` projects durable assistant history by retaining complete, nonduplicate tool calls on the existing `AssistantConversationNativeMessage`, then handing off JSON through `into_value()`. Original tool-call payloads and extensions remain intact. Real results close matching pending calls; orphan or duplicate results follow the existing discard rules. Unmatched calls receive interrupted results at the existing boundaries without executing tools again. Persistence, identity and query authorization remain with their existing owners.

## Public round continuation and compact history proofs

`previous_response_id` identifies a public response round; provider continuation identifies private transport state. Neither substitutes for the other. The semantic path reads a validated v3 history proof from the authorized predecessor, then extends the hash chain with the current input and output. AI Native persists that proof with the tool callback round. Tool-result correlation continues to verify ownership, the exact round, and the complete result set. Legacy rounds missing required proof fail explicitly; a previous answer cannot stand in for complete history evidence.

The predecessor query projects only `version`, `item_count`, `digest`, required identity/status fields, and the answer actually consumed by semantic rendering. Reading proof does not deserialize complete callback or run DTOs, historical output arrays, or tool results. Historical callback facts take precedence over mutable latest run output. Workspace, application, API key, user, and archive visibility checks remain in place. Proof handling adds no full-history cache.

The proof has fixed size; the whole request does not have constant memory cost. Current request bodies, required answers, and provider recovery payloads still consume resources proportional to their contents. Ordinary answers use the scalar projection. Legacy original outputs containing characters PostgreSQL cannot represent use a separate backpressured byte stream and a standard streaming JSON parser in two passes: first locate the last occurrence of the highest-priority field, then extract only that value without constructing a complete output object or caching the original. This exceptional path sequentially transfers the original output twice, and the database may materialize that single stored value; database and network costs are not constant. The proof query itself does not read those originals.
