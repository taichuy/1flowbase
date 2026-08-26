pub const CLAUDE_CODE_COMPACT_SUMMARY_PROMPT_PREFIX: &str =
    "Your task is to create a detailed summary of the conversation so far";
pub const CLAUDE_CODE_PARTIAL_COMPACT_SUMMARY_PROMPT_PREFIX: &str =
    "Your task is to create a detailed summary of the RECENT portion of the conversation";
pub const CLAUDE_CODE_CONTEXT_CONTINUATION_SUMMARY_PROMPT_PREFIX: &str =
    "Your task is to create a detailed summary of this conversation. This summary will be placed at the start of a continuing session";
pub const CLAUDE_CODE_AWAY_SUMMARY_PROMPT_PREFIX: &str =
    "The user stepped away and is coming back. Write exactly 1-3 short sentences.";
pub const CLAUDE_CODE_AWAY_SUMMARY_NEXT_STEP_MARKER: &str = "Next: the concrete next step.";
pub const CLAUDE_CODE_COMPACT_RESUME_MARKER: &str =
    "This session is being continued from a previous conversation that ran out of context.";
pub const CLAUDE_CODE_COMPACT_RESUME_SUMMARY_MARKER: &str =
    "The summary below covers the earlier portion of the conversation.";
pub const CLAUDE_CODE_COMPACT_TRANSCRIPT_MARKER: &str =
    "If you need specific details from before compaction";
pub const CLAUDE_CODE_SESSION_TITLE_SYSTEM_MARKER: &str = "Generate a concise, sentence-case title";
pub const CLAUDE_CODE_SESSION_TITLE_JSON_MARKER: &str = "Return JSON with a single \"title\" field";

pub fn claude_code_control_kind(content: &str) -> Option<&'static str> {
    if content.contains(CLAUDE_CODE_COMPACT_SUMMARY_PROMPT_PREFIX)
        || content.contains(CLAUDE_CODE_PARTIAL_COMPACT_SUMMARY_PROMPT_PREFIX)
        || content.contains(CLAUDE_CODE_CONTEXT_CONTINUATION_SUMMARY_PROMPT_PREFIX)
    {
        return Some("compact_summary");
    }
    if content.contains(CLAUDE_CODE_COMPACT_RESUME_MARKER)
        && (content.contains(CLAUDE_CODE_COMPACT_RESUME_SUMMARY_MARKER)
            || content.contains(CLAUDE_CODE_COMPACT_TRANSCRIPT_MARKER))
    {
        return Some("compact_resume");
    }
    if content.contains(CLAUDE_CODE_AWAY_SUMMARY_PROMPT_PREFIX)
        && content.contains(CLAUDE_CODE_AWAY_SUMMARY_NEXT_STEP_MARKER)
    {
        return Some("away_summary");
    }
    None
}
