# Communication message output contract

[中文](communication-messages.md)

Host and plugin contributions use the same content types and Markdown renderer owned by `extension-contracts` and re-exported by the SDK. The contract covers help, model listings, success/error notices, and plugin replies; formatting is shared across commands and channels.

## Content parameters

| Type | Use | Rendering |
| --- | --- | --- |
| `CommunicationMessageContent::Text` | Literal text, dynamic values, error descriptions | Escape Markdown punctuation; convert LF/CRLF to explicit Markdown hard breaks |
| `CommunicationMessageContent::Blocks` | Parameterized paragraphs and lists | Separate blocks with blank lines; emit `- ` list items and keep continuation lines within their item |
| `CommunicationMessageContent::Markdown` | Authored Markdown, including final AI answers | Preserve the original bytes without guessing the format, escaping again, or replacing line breaks |

`CommunicationMessageBlock` supports `Paragraph` and `BulletList`; `CommunicationMessageInline` supports `Text`, `Strong`, and `Code`. Use `Code` for copyable command usages and model IDs, and `Text` for names and descriptions. The renderer selects code delimiters appropriate for embedded backticks; callers do not pre-escape values.

A Markdown client may collapse a single newline in an ordinary paragraph. Pass separate entries as a `BulletList` instead of joining plain lines.

## Plugin example

```rust,no_run
use runtime_extension_sdk::{
    serve_communication_command, CommunicationMessageBlock as Block,
    CommunicationMessageContent as Content, CommunicationMessageInline as Inline,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    serve_communication_command(std::io::stdin().lock(), std::io::stdout().lock(), |request| {
        Content::Blocks(vec![
            Block::Paragraph(vec![Inline::Strong("Result".into())]),
            Block::BulletList(vec![vec![
                Inline::Code(request.command_id.clone()),
                Inline::Text(format!(" — {}", request.arguments)),
            ]]),
        ])
    })?;
    Ok(())
}
```

A handler may return a `String` or `&str` for a simple `Text` response. Return `Content::markdown(markdown)` explicitly for authored Markdown. General event workers construct `ManagedEventOutcome::CommandReply` with `CommunicationCommandReply::from_content(content)?` and use their existing handler error path when construction fails.

## Wire format, limits, and ownership

The content tree is an author-side API. The wire reply remains `CommunicationCommandReply { text }`, where `text` is rendered Markdown. Command contract `1`, managed-event v2, and persisted `reply_json` retain their existing shape. Callbacks returning an existing `CommunicationCommandReply` remain supported and preserve its Markdown. An old `{text}` value does not reveal whether its author intended plain text or Markdown; old plugins must adopt the content API to obtain explicit plain-text line-break semantics.

Command replies must be nonempty and at most 32,768 UTF-8 bytes **after rendering**. Oversized output returns `CommunicationCommandReplyContentError::TooLong`; the SDK emits neither a partial frame nor truncated code delimiters. The Host shortens long command/model catalogs only at complete-entry boundaries and appends a notice. `render_markdown` and `into_markdown` do not impose the command limit; ordinary final AI answers keep their existing size behavior.

Render once at the producer. Persistence and delivery use the same completed text; channel adapters pass it to the supplier unchanged. Recipient/session binding, permissions, event targets, and delivery receipts remain Host-owned. Formatting grants no additional authority, and attachments keep their existing file protocol.


## Send correlation and provider receipts

A receiving channel declares receipt support with `receive.supports_send_receipts: true`. The flag defaults to `false`, which is omitted on serialization. Only channels declaring this capability receive `NotificationOperation::Send.request_id`, the Host send-log UUID. An omitted request ID keeps the original send protocol. Plugins never declare Host conversation or sender UUIDs.

`NotificationRuntimeResult::Send.receipt` carries a `NotificationSendReceipt` separately from the business `outcome.result`:

| Field | Source and meaning |
| --- | --- |
| `request_id` | Echo the UUID from this Host request unchanged |
| `recipient_id`, `conversation_id` | Actual provider recipient and conversation identities; when `target` exists, match its `sender_id` and `conversation_id` respectively |
| `provider_message_id` | Optional message identity actually issued by the provider |
| `client_message_id` | Optional client identity included in the actual provider request; never substitute it for `provider_message_id` |

Identity fields must be nonempty, contain no control characters, and fit within 1024 UTF-8 bytes. The request ID must also be a UUID. Use `notification_send_receipt(request_id.as_deref(), recipient, conversation, provider_id, client_id)?` for shared construction and field validation. `NotificationSendReceipt::validate_for` checks request and destination correlation, while `execute_communication` validates the descriptor, input, and complete result. Given `request_id: None`, the helper returns `None`, so an old Host receives no new receipt field.

Declare account identities held in private authorization state through optional `authorization.managed_identity_keys`, a unique subset of `managed_secret_keys`. Values must be bounded identity strings (null is allowed for deletion). Storage remains encrypted. Host permits an exact identity only in the standard receipt's `recipient_id` / `conversation_id` fields after validating request and destination correlation. Message text, business results, provider/client message IDs, and other public output remain protected. Passwords, tokens, and undeclared private state are never exempt, even if a value equals an account identity. Older declarations default to an empty list and retain the existing checks.

A successful result for a correlated request requires a matching receipt. `Failed` and `Unknown` may omit it; any supplied receipt must still match the request and target. Undeclared receipt support rejects request IDs, and a result without a request ID cannot return a receipt. Invalid responses and provider errors remain `Unknown` and do not trigger automatic retries. Provider acceptance means accepted submission, not delivered or read. The Host owns correlation and unified send-history archiving across entrypoints.


## Received quotations and history retrieval

The optional `NotificationReceivedMessage.reply_to` contains a `NotificationMessageReference` with optional supplier `provider_message_id`, `title`, and quoted `text`. At least one must be nonempty. IDs retain the 1024-byte identity limit; titles allow up to 1024 bytes and quoted text up to 65536 bytes, within the existing 512 KiB receive-batch budget. Omission preserves the previous receive shape. The wire reference cannot assert a Host message ID, role, credentials, or private download locator. Providers extract quotations separately from the original command text.

Host persists the quotation in the receive transaction. It resolves a unique original message only within the same workspace, channel, sender, and conversation, using an inbound supplier message ID or a stored send receipt's provider/client ID. A verified match adds the Host message_id and original text; otherwise the supplier snapshot is retained without guessing from titles or text. Quoted content reaches the model as user data, never as system instructions or fabricated file attachments. Shared history exposes the same reply_to field for console display.

History tools return at most five messages per page. When looking for an earlier report, follow next_cursor until it is found or history is exhausted; absence from the first page does not establish absence from history. The received-message send_status projection includes the actual channel send associated with that input. Command processing and report sending remain separate outcomes. Existing unknown outcomes without durable receipts remain unknown.
