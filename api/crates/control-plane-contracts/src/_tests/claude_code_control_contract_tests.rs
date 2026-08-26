use crate::application_public_runtime::claude_code_control::{
    claude_code_control_kind, CLAUDE_CODE_AWAY_SUMMARY_NEXT_STEP_MARKER,
    CLAUDE_CODE_AWAY_SUMMARY_PROMPT_PREFIX, CLAUDE_CODE_COMPACT_RESUME_MARKER,
    CLAUDE_CODE_COMPACT_RESUME_SUMMARY_MARKER, CLAUDE_CODE_COMPACT_SUMMARY_PROMPT_PREFIX,
    CLAUDE_CODE_COMPACT_TRANSCRIPT_MARKER, CLAUDE_CODE_CONTEXT_CONTINUATION_SUMMARY_PROMPT_PREFIX,
    CLAUDE_CODE_PARTIAL_COMPACT_SUMMARY_PROMPT_PREFIX, CLAUDE_CODE_SESSION_TITLE_JSON_MARKER,
    CLAUDE_CODE_SESSION_TITLE_SYSTEM_MARKER,
};

#[test]
fn claude_code_control_classification_keeps_markers_and_precedence() {
    for marker in [
        CLAUDE_CODE_COMPACT_SUMMARY_PROMPT_PREFIX,
        CLAUDE_CODE_PARTIAL_COMPACT_SUMMARY_PROMPT_PREFIX,
        CLAUDE_CODE_CONTEXT_CONTINUATION_SUMMARY_PROMPT_PREFIX,
    ] {
        assert_eq!(claude_code_control_kind(marker), Some("compact_summary"));
    }

    for secondary_marker in [
        CLAUDE_CODE_COMPACT_RESUME_SUMMARY_MARKER,
        CLAUDE_CODE_COMPACT_TRANSCRIPT_MARKER,
    ] {
        let content = format!("{CLAUDE_CODE_COMPACT_RESUME_MARKER}\n{secondary_marker}");
        assert_eq!(claude_code_control_kind(&content), Some("compact_resume"));
    }

    let away = format!(
        "{CLAUDE_CODE_AWAY_SUMMARY_PROMPT_PREFIX}\n{CLAUDE_CODE_AWAY_SUMMARY_NEXT_STEP_MARKER}"
    );
    assert_eq!(claude_code_control_kind(&away), Some("away_summary"));
    assert_eq!(
        claude_code_control_kind(&format!(
            "{CLAUDE_CODE_COMPACT_SUMMARY_PROMPT_PREFIX}\n{away}"
        )),
        Some("compact_summary")
    );
}

#[test]
fn claude_code_control_classification_rejects_incomplete_or_unrelated_markers() {
    for content in [
        CLAUDE_CODE_COMPACT_RESUME_MARKER,
        CLAUDE_CODE_COMPACT_RESUME_SUMMARY_MARKER,
        CLAUDE_CODE_COMPACT_TRANSCRIPT_MARKER,
        CLAUDE_CODE_AWAY_SUMMARY_PROMPT_PREFIX,
        CLAUDE_CODE_AWAY_SUMMARY_NEXT_STEP_MARKER,
        CLAUDE_CODE_SESSION_TITLE_SYSTEM_MARKER,
        CLAUDE_CODE_SESSION_TITLE_JSON_MARKER,
        "Your task is to create a beautiful summary of the conversation so far",
    ] {
        assert_eq!(claude_code_control_kind(content), None);
    }
}
