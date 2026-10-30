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
