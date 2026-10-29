use super::*;

#[test]
fn help_uses_literal_usage_and_description_in_independent_list_items() {
    let mut views = plugin_framework::communication_commands::builtin_communication_commands()
        .into_iter()
        .map(|entry| CommunicationCommandView {
            command_id: entry.descriptor.command_id,
            name: entry.descriptor.name,
            description: entry.descriptor.description,
            usage: entry.descriptor.usage,
            source_kind: CommunicationCommandSourceKind::Builtin,
            source_name: entry.source_name,
            installation_id: None,
            enabled: entry.enabled,
            available: entry.available,
            unavailable_reason: None,
        })
        .collect::<Vec<_>>();
    views[1].enabled = false;
    let reply = communication_command_help(&views).unwrap();
    assert_eq!(
        reply.text,
        "- `/new` — Start a new AI session\n- `/help` — Show enabled commands"
    );
    views[0].available = false;
    views[2].description = "literal *value*\nsecond line".into();
    assert_eq!(
        communication_command_help(&views).unwrap().text,
        "- `/help` — literal \\*value\\*\\\n  second line"
    );
}

#[test]
fn long_catalog_omits_complete_entries_and_never_cuts_markdown_or_utf8() {
    let suffix = "说明😀".repeat(80);
    let items = (0..100)
        .map(|index| {
            vec![
                Inline::Code(format!("/entry-{index}`")),
                Inline::Text(format!(" — {suffix}")),
            ]
        })
        .collect::<Vec<_>>();
    let reply = communication_command_list_reply(Vec::new(), items.clone()).unwrap();
    reply.validate().unwrap();
    assert!(reply
        .text
        .ends_with("\n\n… Additional entries are available in channel settings."));
    let lines = reply
        .text
        .lines()
        .filter(|line| line.starts_with("- "))
        .collect::<Vec<_>>();
    assert!(!lines.is_empty() && lines.len() < items.len());
    for (index, line) in lines.iter().enumerate() {
        assert_eq!(*line, format!("- `` /entry-{index}` `` — {suffix}"));
    }
    let next = Content::Blocks(vec![
        Block::BulletList(items[..lines.len() + 1].to_vec()),
        Block::Paragraph(vec![Inline::Text(
            "… Additional entries are available in channel settings.".into(),
        )]),
    ]);
    assert!(matches!(
        CommunicationCommandReply::from_content(next),
        Err(CommunicationCommandReplyContentError::TooLong { .. })
    ));
}

#[test]
fn oversized_first_entry_becomes_notice_instead_of_broken_inline_code() {
    let reply =
        communication_command_list_reply(Vec::new(), vec![vec![Inline::Code("`".repeat(40_000))]])
            .unwrap();
    assert_eq!(
        reply.text,
        "… Additional entries are available in channel settings."
    );
    assert_eq!(
        communication_command_help(&[]).unwrap().text,
        "No commands are currently enabled."
    );
}
