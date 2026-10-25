# Communications center

[简体中文](README.md)

The communications center manages channels, conversations, received files, and delivery state at `/settings/communications`. In-app notifications retain their recipient inbox.

## Configuration and messages

Receive-capable plugins declare separate receive/send contributions, a default prompt, and MCP tools. The host provisions a dedicated MCP instance when a channel is created. Agent-flow bindings select it by default and can include additional instances. Application model and reasoning choices use the same capabilities as the AI assistant. An unset file table uses file management's default table.

Each communication run starts with three logical messages: stable application and plugin instructions, separate host-generated session context, and the current user message. Users can edit the plugin prompt. History is not automatically replayed: the model discovers and invokes MCP tools progressively to retrieve messages and files. Providers may represent system instructions differently. A stable prefix can help prompt caching but does not guarantee a hit.

## Receipt and recovery

Plugins return a message batch and candidate cursor. One host transaction deduplicates and persists the inbox, encrypted attachment references, authorization state, and cursor. A failed transaction does not advance the cursor. Receive, send, and authorization maintenance must not independently consume the same provider cursor.

Messages are claimed in conversation order with renewable leases. Attachments use bounded plugin transfer chunks and the existing file management upload service. Failures remain visible; the backend determines retry eligibility. An external send with an uncertain result becomes unknown and is not automatically retried.

Provider download tokens, encryption keys, and account credentials never enter public records, prompts, or MCP results.

## MCP and replies

Plugin declarations create real MCP instances, tools, and bindings under the existing discovery policy. The host binds workspace, channel, and external identity to each communication invocation. Model arguments can select conversations and files within that scope, never expand it.

Reply destinations come from the committed inbound sender and conversation. Later messages cannot change them. Successful execution uses the canonical business-conversation final-answer projection. Reasoning, tool output, callback waits, failures, and timeout placeholders are not sent as replies.

## Evidence

[Root #2670](https://github.com/taichuy/1flowbase/issues/2670) owns acceptance and evidence. Protocol mocks, real database tests, and live WeChat delivery are reported separately; a simulated success does not prove delivery to WeChat.
