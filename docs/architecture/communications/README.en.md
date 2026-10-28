# Communications center

[简体中文](README.md)

The communications center manages channels, conversations, received files, and delivery state at `/settings/communications`. In-app notifications retain their recipient inbox.

## Configuration and messages

Receive-capable plugins declare separate receive/send contributions, a default prompt, and MCP tools. The host provisions a dedicated MCP instance when a channel is created. Agent-flow bindings select it by default and can include additional instances. Application model and reasoning choices use the same capabilities as the AI assistant. An unset file table uses file management's default table.

Each communication run starts with three logical messages: stable application and plugin instructions, separate host-generated session context, and the current user message. Users can edit the plugin prompt. History is not automatically replayed: the model discovers and invokes MCP tools progressively to retrieve messages and files. Providers may represent system instructions differently. A stable prefix can help prompt caching but does not guarantee a hit.

## Receipt and recovery

Plugins return a message batch and candidate cursor. One host transaction deduplicates and persists the inbox, encrypted attachment references, authorization state, and cursor. A failed transaction does not advance the cursor. Receive, send, and authorization maintenance must not independently consume the same provider cursor.

External-chat history, AI sessions, reply turns, and workflow runs have separate identities. A chat reuses its AI session, and one reply turn can contain several individually retained inbound messages. Different chats execute independently. Attachments use bounded plugin transfer chunks and the existing file management upload service. Failures remain visible; the backend determines retry eligibility. An external send with an uncertain result becomes unknown and is not automatically retried.

Provider download tokens, encryption keys, and account credentials never enter public records, prompts, or MCP results.

## Queueing and additional input

Ordinary messages are persisted before admission to the current reply turn, in receipt order. Input received during generation is consumed before a model call, after complete tool results, or after a candidate final answer. Admission and final send reservation serialize on the same chat lock: input received before reservation belongs to the current turn; later input starts the next turn.

Continuation retains the model transcript and completed tool results. It does not redispatch the workflow or replay completed side effects. Human approval and external-tool callback resumes retain the turn, claim, and unfinished model node. The final Answer must be refreshable from the retained model output through pure projections. If it depends on completed work that cannot safely be recomputed, continuation fails explicitly rather than discarding the new input or repeating external actions.

Input consumption and continuation state commit atomically. Restart can recover queued runs and existing wait points. Runs with uncertain execution results are not automatically replayed. Existing history, channels, authorization, and provider cursors keep their identities.

## Chat commands

The channel binding form lists commands, sources, descriptions, switches, and availability reasons. Built-in commands are enabled by default:

| Command | Behavior |
| --- | --- |
| `/new` | Stop the old session, fence unsent replies, and start a new AI session |
| `/model` | List models allowed by the bound application |
| `/model-user-<model ID>` or `/model <model ID>` | Select a model for this external chat's future turns |
| `/help` | List commands that are enabled and available for this channel |

`/new` uses an independent control lane. The host durably invalidates the old session, cancels and joins its execution, then replies: `The previous conversation has stopped generating. A new conversation has started.` Completed external effects and messages already submitted to a provider cannot be recalled. The chat's model choice survives `/new`; it changes neither the channel default nor a running turn.

Unknown, disabled, unavailable, or invalid commands receive explicit responses and never become Agent-flow prompts. Their original text remains in history. Custom workflow-bound commands are not available yet.

Plugins declare commands globally in `PluginManifestV1.communication_commands`, independently of receive/send capabilities. Each declaration binds one `1flowbase.communication.command.requested` event subscriber contribution and uses a `publisher/plugin/local` command ID. The host reserves `new`, `model`, `help`, and `model-user-*` names.

Commands first commit structured durable events, freezing one execution target and its authorization version. Disabled, removed, or unauthorized plugins cannot execute them. A plugin returns `CommunicationCommandReply { text }`; the host derives the destination from the inbound record. Effects, reply receipts, and provider delivery have separate outcomes: handler ACK does not imply delivery. Redelivery reuses the same result receipt, without promising cross-system exactly-once execution for arbitrary external plugin effects.

## MCP and replies

Plugin declarations create real MCP instances, tools, and bindings under the existing discovery policy. The host binds workspace, channel, and external identity to each communication invocation. Model arguments can select conversations and files within that scope, never expand it.

Reply destinations come from the committed inbound sender and conversation. Later messages cannot change them. Successful execution uses the canonical business-conversation final-answer projection. Reasoning, tool output, callback waits, failures, and timeout placeholders are not sent as replies.

## Evidence

[Root #2670](https://github.com/taichuy/1flowbase/issues/2670) owns receipt/history acceptance, and [Root #2695](https://github.com/taichuy/1flowbase/issues/2695) owns sessions, appendable turns, and commands. Protocol mocks, real database tests, and live WeChat delivery are reported separately; a simulated success does not prove delivery to WeChat.
