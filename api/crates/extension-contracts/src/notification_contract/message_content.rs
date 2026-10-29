//! Author-side content rendered once into the existing communications `text` wire field.
use std::borrow::Cow;

use thiserror::Error;

use super::{CommunicationCommandReply, COMMUNICATION_COMMAND_REPLY_MAX_BYTES};

/// Explicit author intent; this is not an additional wire format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommunicationMessageContent {
    /// Literal text with Markdown punctuation escaped and visible line breaks preserved.
    Text(String),
    /// Already authored Markdown, preserved byte-for-byte without format detection.
    Markdown(String),
    /// Paragraphs and lists assembled by the canonical Markdown renderer.
    Blocks(Vec<CommunicationMessageBlock>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommunicationMessageBlock {
    Paragraph(Vec<CommunicationMessageInline>),
    BulletList(Vec<Vec<CommunicationMessageInline>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommunicationMessageInline {
    Text(String),
    Strong(String),
    /// Literal code. Multiline values use one code span per line and hard breaks.
    Code(String),
}

/// Command limits apply to the rendered UTF-8 wire value, never to source characters.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CommunicationCommandReplyContentError {
    #[error("command reply requires non-empty content")]
    Empty,
    #[error("rendered command reply has {bytes} bytes, exceeding the {max_bytes}-byte limit")]
    TooLong { bytes: usize, max_bytes: usize },
}

impl CommunicationMessageContent {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub fn markdown(markdown: impl Into<String>) -> Self {
        Self::Markdown(markdown.into())
    }

    /// Render without a command-specific size limit, including for existing AI Markdown.
    pub fn render_markdown(&self) -> String {
        match self {
            Self::Text(text) => render_runs(std::iter::once((InlineStyle::Text, text.as_str()))),
            Self::Markdown(markdown) => markdown.clone(),
            Self::Blocks(blocks) => blocks
                .iter()
                .map(render_block)
                .filter(|block| !block.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n"),
        }
    }

    /// Preserve already rendered Markdown by moving its original buffer unchanged.
    pub fn into_markdown(self) -> String {
        match self {
            Self::Markdown(markdown) => markdown,
            content => content.render_markdown(),
        }
    }

    fn is_blank(&self) -> bool {
        match self {
            Self::Text(text) | Self::Markdown(text) => text.trim().is_empty(),
            Self::Blocks(blocks) => blocks.iter().all(|block| match block {
                CommunicationMessageBlock::Paragraph(inlines) => inlines_are_blank(inlines),
                CommunicationMessageBlock::BulletList(items) => {
                    items.iter().all(|inlines| inlines_are_blank(inlines))
                }
            }),
        }
    }
}

impl From<String> for CommunicationMessageContent {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<&str> for CommunicationMessageContent {
    fn from(text: &str) -> Self {
        Self::text(text)
    }
}

impl From<CommunicationCommandReply> for CommunicationMessageContent {
    fn from(reply: CommunicationCommandReply) -> Self {
        // Legacy wire replies already contain rendered text; never guess their source format.
        Self::Markdown(reply.text)
    }
}

impl CommunicationCommandReply {
    pub fn from_content(
        content: impl Into<CommunicationMessageContent>,
    ) -> Result<Self, CommunicationCommandReplyContentError> {
        let content = content.into();
        // Escapes, code delimiters and list markers must not make blank source content valid.
        if content.is_blank() {
            return Err(CommunicationCommandReplyContentError::Empty);
        }
        let text = content.into_markdown();
        if text.len() > COMMUNICATION_COMMAND_REPLY_MAX_BYTES {
            return Err(CommunicationCommandReplyContentError::TooLong {
                bytes: text.len(),
                max_bytes: COMMUNICATION_COMMAND_REPLY_MAX_BYTES,
            });
        }
        Ok(Self { text })
    }
}

fn inlines_are_blank(inlines: &[CommunicationMessageInline]) -> bool {
    inlines
        .iter()
        .all(|inline| inline_value(inline).trim().is_empty())
}

fn inline_value(inline: &CommunicationMessageInline) -> &str {
    match inline {
        CommunicationMessageInline::Text(text)
        | CommunicationMessageInline::Strong(text)
        | CommunicationMessageInline::Code(text) => text,
    }
}

fn render_block(block: &CommunicationMessageBlock) -> String {
    match block {
        CommunicationMessageBlock::Paragraph(inlines) => render_inlines(inlines),
        CommunicationMessageBlock::BulletList(items) => items
            .iter()
            .map(|inlines| {
                let item = render_inlines(inlines);
                if item.is_empty() {
                    "- &#32;".to_owned()
                } else {
                    // Continuations belong to the same item, including hard breaks in code.
                    format!("- {}", item.replace('\n', "\n  "))
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InlineStyle {
    Text,
    Strong,
    Code,
}

fn render_inlines(inlines: &[CommunicationMessageInline]) -> String {
    render_runs(inlines.iter().map(|inline| {
        let style = match inline {
            CommunicationMessageInline::Text(_) => InlineStyle::Text,
            CommunicationMessageInline::Strong(_) => InlineStyle::Strong,
            CommunicationMessageInline::Code(_) => InlineStyle::Code,
        };
        (style, inline_value(inline))
    }))
}

fn render_runs<'a>(values: impl IntoIterator<Item = (InlineStyle, &'a str)>) -> String {
    let mut runs: Vec<(InlineStyle, Cow<'_, str>)> = Vec::new();
    for (style, value) in values {
        if value.is_empty() {
            continue;
        }
        if let Some((previous_style, previous_value)) = runs.last_mut() {
            if *previous_style == style {
                // Adjacent values are one visible run: splitting must not create delimiters
                // or hide a list marker assembled from separate text parameters.
                previous_value.to_mut().push_str(value);
                continue;
            }
        }
        runs.push((style, Cow::Borrowed(value)));
    }
    let mut markdown = String::new();
    for (index, (style, value)) in runs.iter().enumerate() {
        let follows_strong = index > 0 && runs[index - 1].0 == InlineStyle::Strong;
        let precedes_strong = runs
            .get(index + 1)
            .is_some_and(|run| run.0 == InlineStyle::Strong);
        let normalized = if value.contains('\r') {
            Cow::Owned(value.replace("\r\n", "\n").replace('\r', "\n"))
        } else {
            Cow::Borrowed(value.as_ref())
        };
        for (line_index, line) in normalized.split('\n').enumerate() {
            if line_index > 0 {
                markdown.push_str("\\\n");
            }
            match style {
                InlineStyle::Text => {
                    // Entity boundaries keep strong delimiters valid even between word
                    // characters and punctuation, without inserting visible whitespace.
                    escape_line(line, &mut markdown, follows_strong, precedes_strong);
                }
                InlineStyle::Strong if !line.trim().is_empty() => {
                    markdown.push_str("**");
                    let first_is_space = line.chars().next().is_some_and(char::is_whitespace);
                    let last_is_space = line.chars().next_back().is_some_and(char::is_whitespace);
                    escape_line(line, &mut markdown, first_is_space, last_is_space);
                    markdown.push_str("**");
                }
                InlineStyle::Strong => escape_line(line, &mut markdown, false, false),
                InlineStyle::Code => render_code_line(line, &mut markdown),
            }
        }
    }
    if markdown.ends_with("\\\n") {
        // CommonMark drops a hard break at the end of a block. A whitespace entity
        // keeps the last empty line visible without displaying a dangling backslash.
        markdown.push_str("&#32;");
    }
    markdown
}

fn escape_line(line: &str, markdown: &mut String, encode_first: bool, encode_last: bool) {
    let mut chars = line.chars().enumerate().peekable();
    let mut at_line_start = markdown.is_empty() || markdown.ends_with('\n');
    while let Some((index, character)) = chars.next() {
        if (encode_first && index == 0)
            || (encode_last && chars.peek().is_none())
            || (at_line_start && matches!(character, ' ' | '\t'))
            // GFM also recognizes bare URLs and email addresses. Encoding the
            // token boundary preserves literal punctuation without letting an
            // autolink consume a following escape or hard break.
            || character == '@'
            || (character == ':' && chars.peek().is_some_and(|(_, next)| *next == '/'))
            || (character == '.'
                && markdown.as_bytes().last().is_some_and(|last| matches!(last, b'w' | b'W')))
        {
            markdown.push_str(&format!("&#{};", character as u32));
        } else {
            if matches!(
                character,
                '\\' | '`'
                    | '*'
                    | '_'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | '('
                    | ')'
                    | '<'
                    | '>'
                    | '#'
                    | '+'
                    | '-'
                    | '!'
                    | '|'
                    | '~'
                    | '='
                    | '&'
            ) || (character == '.' && ends_with_ordered_list_prefix(markdown))
            {
                markdown.push('\\');
            }
            markdown.push(character);
        }
        at_line_start &= matches!(character, ' ' | '\t');
    }
}

fn ends_with_ordered_list_prefix(markdown: &str) -> bool {
    // CommonMark markers contain at most nine digits; avoid rescanning long lines.
    let bytes = markdown.as_bytes();
    let tail = &bytes[bytes.len().saturating_sub(10)..];
    let line = tail
        .rsplit(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    (1..=9).contains(&line.len()) && line.iter().all(|byte| byte.is_ascii_digit())
}

fn render_code_line(line: &str, markdown: &mut String) {
    if line.is_empty() {
        return;
    }
    let longest_run = line
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest_run + 1);
    let pad = line.starts_with('`')
        || line.ends_with('`')
        || (line.starts_with(' ') && line.ends_with(' ') && !line.bytes().all(|byte| byte == b' '));
    markdown.push_str(&fence);
    if pad {
        markdown.push(' ');
    }
    markdown.push_str(line);
    if pad {
        markdown.push(' ');
    }
    markdown.push_str(&fence);
}
