use crate::{
    CommunicationCommandReply, CommunicationCommandReplyContentError,
    CommunicationMessageBlock as Block, CommunicationMessageContent as Content,
    CommunicationMessageInline as Inline, COMMUNICATION_COMMAND_CONTRACT_VERSION,
    COMMUNICATION_COMMAND_REPLY_MAX_BYTES,
};
use serde_json::{json, Value};

fn fixture(name: &str) -> String {
    // Root's Markdown parser checks these same outputs after the Rust assertions pass.
    let fixtures: Value =
        serde_json::from_str(include_str!("message_content_fixtures.json")).unwrap();
    fixtures[name].as_str().unwrap().to_owned()
}

#[test]
fn notification_text_preserves_visible_lf_crlf_and_unicode() {
    let content = Content::text("你好👋\n第二行\r\n第三行");
    assert_eq!(content.render_markdown(), fixture("plain_multiline"));
    assert_eq!(content.into_markdown(), fixture("plain_multiline"));
    assert_eq!(Content::text("a\rb").render_markdown(), "a\\\nb");
    assert_eq!(
        Content::text("a\n\nb\n").render_markdown(),
        "a\\\n\\\nb\\\n&#32;"
    );
}

#[test]
fn notification_plain_english_sentences_and_string_conversions_preserve_bytes() {
    for text in [
        "New conversation started.",
        "Plugin is ready.",
        "Model selection is not available for this channel.",
    ] {
        assert_eq!(Content::from(text).render_markdown(), text);
        assert_eq!(Content::from(text.to_owned()).into_markdown(), text);
        assert_eq!(
            CommunicationCommandReply::from_content(text).unwrap().text,
            text
        );
    }
}

#[test]
fn notification_literal_text_cannot_create_markdown_structure_or_html() {
    let content = Content::text(
        "# title\n- [link](https://example.com)\n<script>alert(1)</script>\n&copy;\n===",
    );
    assert_eq!(content.render_markdown(), fixture("literal_injection"));
    assert_eq!(
        Content::text("*bold* _em_ ~strike~ |table| \\ `code`").render_markdown(),
        "\\*bold\\* \\_em\\_ \\~strike\\~ \\|table\\| \\\\ \\`code\\`"
    );
    assert_eq!(
        Content::text("    indented\n\tcode").render_markdown(),
        "&#32;&#32;&#32;&#32;indented\\\n&#9;code"
    );
}

#[test]
fn notification_blocks_form_paragraphs_and_complete_bullet_items() {
    let content = Content::Blocks(vec![
        Block::Paragraph(vec![Inline::Strong("Available commands".into())]),
        Block::BulletList(vec![
            vec![
                Inline::Code("/help".into()),
                Inline::Text(" — Show available commands.".into()),
            ],
            vec![
                Inline::Code("/model".into()),
                Inline::Text(" — Select a model.".into()),
            ],
        ]),
    ]);
    assert_eq!(content.render_markdown(), fixture("structured_commands"));
}

#[test]
fn notification_adjacent_text_runs_cannot_assemble_ordered_list_markers() {
    let content = Content::Blocks(vec![Block::Paragraph(vec![
        Inline::Text("1".into()),
        Inline::Text(".".into()),
        Inline::Text(" injected".into()),
    ])]);
    assert_eq!(content.render_markdown(), fixture("ordered_injection"));
    assert_eq!(
        Content::text("123456789. item\n1) item\nVersion 1.0.").render_markdown(),
        "123456789\\. item\\\n1\\) item\\\nVersion 1.0."
    );
}

#[test]
fn notification_code_uses_safe_delimiters_and_preserves_edge_spaces() {
    let content = Content::Blocks(vec![Block::Paragraph(vec![
        Inline::Code("alpha`beta".into()),
        Inline::Text(" / ".into()),
        Inline::Code("`edge`".into()),
        Inline::Text(" / ".into()),
        Inline::Code(" both ".into()),
        Inline::Text(" / ".into()),
        Inline::Code("   ".into()),
    ])]);
    assert_eq!(content.render_markdown(), fixture("code_fences"));
    for (literal, expected) in [
        ("a``b", "```a``b```"),
        ("`", "`` ` ``"),
        ("` `", "`` ` ` ``"),
        (" leading", "` leading`"),
        ("trailing ", "`trailing `"),
        ("\t", "`\t`"),
    ] {
        assert_eq!(
            Content::Blocks(vec![Block::Paragraph(vec![Inline::Code(literal.into())])])
                .render_markdown(),
            expected,
            "{literal:?}"
        );
    }
}

#[test]
fn notification_adjacent_code_values_and_empty_inlines_cannot_create_delimiters() {
    assert_eq!(
        Content::Blocks(vec![Block::Paragraph(vec![
            Inline::Code("a`".into()),
            Inline::Code("".into()),
            Inline::Code("`b".into()),
        ])])
        .render_markdown(),
        "```a``b```"
    );
    assert_eq!(
        Content::Blocks(vec![Block::Paragraph(vec![
            Inline::Text("1".into()),
            Inline::Code("".into()),
            Inline::Text(". injected".into()),
        ])])
        .render_markdown(),
        fixture("ordered_injection")
    );
}

#[test]
fn notification_strong_handles_punctuation_whitespace_and_adjacent_runs() {
    let content = Content::Blocks(vec![Block::Paragraph(vec![
        Inline::Text("abc".into()),
        Inline::Strong("[name]".into()),
        Inline::Text("xyz".into()),
    ])]);
    assert_eq!(content.render_markdown(), fixture("strong_edges"));
    assert_eq!(
        Content::Blocks(vec![Block::Paragraph(vec![
            Inline::Strong(" first ".into()),
            Inline::Strong("second\n third ".into()),
        ])])
        .render_markdown(),
        "**&#32;first second**\\\n**&#32;third&#32;**"
    );
}

#[test]
fn notification_multiline_list_text_and_code_stay_inside_their_items() {
    let content = Content::Blocks(vec![Block::BulletList(vec![
        vec![Inline::Text("first\nsecond\r\n- injected".into())],
        vec![Inline::Code("`x`\r\ny".into())],
    ])]);
    assert_eq!(content.render_markdown(), fixture("list_multiline"));
}

#[test]
fn notification_explicit_markdown_and_legacy_reply_preserve_wire_bytes() {
    let markdown = "# 标题 👋\r\n\r\n- **first**\r\n- second\r\n\r\n| a | b |\n| - | - |\n| 1 | 2 |\n\n```rust\nlet value = `raw`;\n```\n";
    assert_eq!(Content::markdown(markdown).render_markdown(), markdown);
    assert_eq!(Content::markdown(markdown).into_markdown(), markdown);
    let reply = CommunicationCommandReply::from_content(Content::markdown(markdown)).unwrap();
    assert_eq!(
        serde_json::to_value(&reply).unwrap(),
        json!({ "text": markdown })
    );
    assert_eq!(Content::from(reply.clone()).into_markdown(), markdown);
    assert_eq!(
        CommunicationCommandReply::from_content(reply.clone()).unwrap(),
        reply
    );
    assert_eq!(COMMUNICATION_COMMAND_CONTRACT_VERSION, "1");
    assert_eq!(
        serde_json::from_value::<CommunicationCommandReply>(json!({"text": markdown})).unwrap(),
        reply
    );
}

#[test]
fn notification_reply_rejects_semantically_empty_content_before_escaping() {
    for content in [
        Content::text(""),
        Content::text(" \r\n\t"),
        Content::markdown(" \r\n\t"),
        Content::Blocks(vec![]),
        Content::Blocks(vec![Block::Paragraph(vec![])]),
        Content::Blocks(vec![Block::BulletList(vec![])]),
        Content::Blocks(vec![Block::BulletList(vec![vec![Inline::Code(
            " ".into(),
        )]])]),
        Content::Blocks(vec![Block::Paragraph(vec![Inline::Strong(" \n ".into())])]),
    ] {
        assert_eq!(
            CommunicationCommandReply::from_content(content),
            Err(CommunicationCommandReplyContentError::Empty)
        );
    }
}

#[test]
fn notification_reply_checks_rendered_utf8_bytes_without_truncating_markdown() {
    let limit = COMMUNICATION_COMMAND_REPLY_MAX_BYTES;
    assert_eq!(limit, 32_768);
    for text in ["x".repeat(limit), "🙂".repeat(limit / 4)] {
        assert_eq!(
            CommunicationCommandReply::from_content(text.clone())
                .unwrap()
                .text,
            text
        );
    }
    assert_eq!(
        CommunicationCommandReply::from_content("🙂".repeat(limit / 4 + 1)),
        Err(CommunicationCommandReplyContentError::TooLong {
            bytes: limit + 4,
            max_bytes: limit,
        })
    );
    assert_eq!(
        CommunicationCommandReply::from_content("*".repeat(limit / 2 + 1)),
        Err(CommunicationCommandReplyContentError::TooLong {
            bytes: limit + 2,
            max_bytes: limit,
        })
    );
    assert_eq!(
        CommunicationCommandReply::from_content("*".repeat(limit / 2))
            .unwrap()
            .text
            .len(),
        limit
    );
    assert_eq!(
        CommunicationCommandReply::from_content(Content::Blocks(vec![Block::BulletList(vec![
            vec![Inline::Text("x".repeat(limit - 1))],
        ])])),
        Err(CommunicationCommandReplyContentError::TooLong {
            bytes: limit + 1,
            max_bytes: limit,
        })
    );
    let markdown = format!("```\n{}\n```", "x".repeat(limit));
    assert_eq!(
        CommunicationCommandReply::from_content(Content::markdown(markdown.clone())),
        Err(CommunicationCommandReplyContentError::TooLong {
            bytes: markdown.len(),
            max_bytes: limit,
        })
    );
    // AI output uses the renderer directly and retains its existing, separate limit.
    assert_eq!(
        Content::markdown(markdown.clone()).into_markdown(),
        markdown
    );
}
