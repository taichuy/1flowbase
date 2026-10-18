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
