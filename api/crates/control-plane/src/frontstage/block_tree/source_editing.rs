use super::*;
use serde::Serialize;

const MAX_SEARCH_MATCHES: usize = 100;
const MAX_DIFF_CHARS: usize = 8_000;
const MAX_TEXT_EDIT_BYTES: usize = 1_000_000;

pub struct SearchFrontstageBlockCodeCommand {
    pub scope: FrontstageBlockScopeCommand,
    pub query: String,
    pub start_line: u32,
    pub start_column: u32,
    pub limit: u32,
    pub expected_source_revision: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FrontstageSourceTextEdit {
    pub old_text: String,
    pub new_text: String,
}

pub struct ReplaceFrontstageBlockCodeCommand {
    pub scope: FrontstageBlockScopeCommand,
    pub expected_source_revision: String,
    pub edits: Vec<FrontstageSourceTextEdit>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourcePosition {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug)]
pub struct FrontstageSourceMatch {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub context: String,
    pub context_start_line: u32,
    pub context_start_column: u32,
    pub context_truncated: bool,
}

#[derive(Debug)]
pub struct FrontstageCodeSearch {
    pub block_id: String,
    pub page_id: Uuid,
    pub source_revision: String,
    pub matches: Vec<FrontstageSourceMatch>,
    pub truncated: bool,
    pub next_line: Option<u32>,
    pub next_column: Option<u32>,
}

#[derive(Debug)]
pub struct FrontstageSourceChange {
    pub edit_index: usize,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub old_text: String,
    pub new_text: String,
    pub truncated: bool,
}

#[derive(Debug)]
pub struct FrontstageCodeEditReceipt {
    pub block_id: String,
    pub page_id: Uuid,
    pub source_revision: String,
    pub applied_edits: usize,
    pub changes: Vec<FrontstageSourceChange>,
    pub diff_truncated: bool,
}

#[derive(Debug, thiserror::Error, Serialize)]
#[error("{code}")]
pub struct FrontstageSourceEditError {
    pub code: &'static str,
    pub edit_index: Option<usize>,
    pub current_source_revision: Option<String>,
    pub candidates: Vec<SourcePosition>,
    pub candidates_truncated: bool,
}

impl FrontstageSourceEditError {
    fn edit(code: &'static str, edit_index: usize) -> Self {
        Self {
            code,
            edit_index: Some(edit_index),
            current_source_revision: None,
            candidates: Vec::new(),
            candidates_truncated: false,
        }
    }

    pub fn conflict(current_source_revision: Option<String>) -> Self {
        Self {
            code: "frontstage_block_source_revision",
            edit_index: None,
            current_source_revision,
            candidates: Vec::new(),
            candidates_truncated: false,
        }
    }
}

// The original indices survive sorting so diagnostics refer to the caller's batch.
struct SourceReplacement {
    index: usize,
    start: usize,
    end: usize,
    replacement: String,
}

impl<R> FrontstagePageService<R>
where
    R: FrontstagePageRepository + FrontstageBlockTreeRepository,
{
    pub async fn search_block_code(
        &self,
        command: SearchFrontstageBlockCodeCommand,
    ) -> Result<FrontstageCodeSearch> {
        let block_id = command.scope.block_id.clone();
        let code = self.get_block_node_code(command.scope).await?;
        if let Some(expected) = validate_source_revision(command.expected_source_revision)? {
            if code.source_sha256.as_deref() != Some(&expected) {
                return Err(FrontstageSourceEditError::conflict(code.source_sha256).into());
            }
        }
        search_source(
            block_id,
            code,
            &command.query,
            command.start_line,
            command.start_column,
            command.limit,
        )
    }

    pub async fn replace_block_code(
        &self,
        command: ReplaceFrontstageBlockCodeCommand,
    ) -> Result<FrontstageCodeEditReceipt> {
        let current = self
            .load_source_for_edit(&command.scope, &command.expected_source_revision)
            .await?;
        let ranges = text_replacements(&current.source_code, command.edits)?;
        self.commit_source_edits(
            command.scope,
            command.expected_source_revision,
            current,
            ranges,
        )
        .await
    }

    pub async fn patch_block_node_code(
        &self,
        command: PatchFrontstageBlockNodeCodeCommand,
    ) -> Result<FrontstageCodeEditReceipt> {
        let current = self
            .load_source_for_edit(&command.scope, &command.expected_source_revision)
            .await?;
        if command.edits.is_empty() || command.edits.len() > MAX_SOURCE_EDITS {
            return Err(ControlPlaneError::InvalidInput("edits").into());
        }
        let starts = source_line_starts(&current.source_code);
        let ranges = command
            .edits
            .into_iter()
            .enumerate()
            .map(|(index, edit)| {
                let position = |line, column| {
                    source_position_offset(&current.source_code, &starts, line, column)
                        .map_err(|_| FrontstageSourceEditError::edit("source_position", index))
                };
                let start = position(edit.start_line, edit.start_column)?;
                let end = position(edit.end_line, edit.end_column)?;
                Ok(SourceReplacement {
                    index,
                    start,
                    end,
                    replacement: edit.replacement,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.commit_source_edits(
            command.scope,
            command.expected_source_revision,
            current,
            ranges,
        )
        .await
    }

    async fn load_source_for_edit(
        &self,
        scope: &FrontstageBlockScopeCommand,
        revision: &str,
    ) -> Result<domain::frontstage::FrontstageBlockCodeRecord> {
        self.ensure_block_designer(scope.actor_user_id, scope.workspace_id, scope.page_id, None)
            .await?;
        let node = self.load_block_node(scope).await?;
        validate_source_revision(Some(revision.to_owned()))?;
        let current = self
            .repository
            .get_frontstage_block_code(scope.workspace_id, scope.page_id, &node.code_ref)
            .await?
            .ok_or(ControlPlaneError::NotFound("block_node_not_found"))?;
        if current.source_sha256.as_deref() != Some(revision) {
            return Err(FrontstageSourceEditError::conflict(current.source_sha256).into());
        }
        Ok(current)
    }

    async fn commit_source_edits(
        &self,
        scope: FrontstageBlockScopeCommand,
        revision: String,
        current: domain::frontstage::FrontstageBlockCodeRecord,
        ranges: Vec<SourceReplacement>,
    ) -> Result<FrontstageCodeEditReceipt> {
        let (source_code, changes) = apply_replacements(&current.source_code, ranges)?;
        let block_id = scope.block_id.clone();
        let page_id = scope.page_id;
        // Reuse the canonical write owner, including its SQL compare-and-swap and audit transaction.
        let saved = self
            .save_block_node_code(SaveFrontstageBlockNodeCodeCommand {
                scope: FrontstageBlockScopeCommand {
                    actor_user_id: scope.actor_user_id,
                    workspace_id: scope.workspace_id,
                    page_id,
                    block_id: block_id.clone(),
                },
                expected_source_revision: Some(revision),
                source_code,
            })
            .await;
        let saved = match saved {
            Ok(saved) => saved,
            Err(error)
                if matches!(
                    error.downcast_ref::<ControlPlaneError>(),
                    Some(ControlPlaneError::Conflict(
                        "frontstage_block_source_revision"
                    ))
                ) =>
            {
                // A concurrent write may win after load_source_for_edit. Re-read only after rechecking visibility.
                let latest = self.get_block_node_code(scope).await?;
                return Err(FrontstageSourceEditError::conflict(latest.source_sha256).into());
            }
            Err(error) => return Err(error),
        };
        Ok(FrontstageCodeEditReceipt {
            block_id,
            page_id,
            source_revision: saved
                .source_sha256
                .ok_or(ControlPlaneError::InvalidInput("source_revision"))?,
            applied_edits: changes.len(),
            diff_truncated: changes.iter().any(|change| change.truncated),
            changes,
        })
    }
}

fn position_at(source: &str, starts: &[usize], offset: usize) -> SourcePosition {
    let line = starts.partition_point(|start| *start <= offset) - 1;
    SourcePosition {
        line: (line + 1) as u32,
        column: (source[starts[line]..offset].chars().count() + 1) as u32,
    }
}

// Advance by one scalar after a hit to detect overlapping occurrences ("aa" in "aaa").
fn matching_offsets<'a>(
    source: &'a str,
    query: &'a str,
    start: usize,
) -> impl Iterator<Item = usize> + 'a {
    let mut cursor = start;
    std::iter::from_fn(move || {
        let offset = cursor + source.get(cursor..)?.find(query)?;
        cursor = offset + source[offset..].chars().next()?.len_utf8();
        Some(offset)
    })
}

fn search_source(
    block_id: String,
    code: domain::frontstage::FrontstageBlockCodeRecord,
    query: &str,
    start_line: u32,
    start_column: u32,
    limit: u32,
) -> Result<FrontstageCodeSearch> {
    if query.is_empty() || query.len() > 8_000 {
        return Err(ControlPlaneError::InvalidInput("query").into());
    }
    if limit == 0 || limit as usize > MAX_SEARCH_MATCHES {
        return Err(ControlPlaneError::InvalidInput("limit").into());
    }
    let source = &code.source_code;
    let starts = source_line_starts(source);
    let start = source_position_offset(source, &starts, start_line, start_column)?;
    let offsets = matching_offsets(source, query, start)
        .take(limit as usize + 1)
        .collect::<Vec<_>>();
    let next = offsets
        .get(limit as usize)
        .map(|offset| position_at(source, &starts, *offset));
    let matches = offsets
        .into_iter()
        .take(limit as usize)
        .map(|offset| {
            let begin = position_at(source, &starts, offset);
            let end = position_at(source, &starts, offset + query.len());
            let context_start = source[..offset]
                .char_indices()
                .rev()
                .nth(79)
                .map(|(i, _)| i)
                .unwrap_or(0);
            let context = source[context_start..]
                .chars()
                .take(320)
                .collect::<String>();
            let context_end = context_start + context.len();
            let context_position = position_at(source, &starts, context_start);
            FrontstageSourceMatch {
                start_line: begin.line,
                start_column: begin.column,
                end_line: end.line,
                end_column: end.column,
                context,
                context_start_line: context_position.line,
                context_start_column: context_position.column,
                context_truncated: context_start > 0 || context_end < source.len(),
            }
        })
        .collect();
    Ok(FrontstageCodeSearch {
        block_id,
        page_id: code.page_id,
        source_revision: code
            .source_sha256
            .ok_or(ControlPlaneError::InvalidInput("source_revision"))?,
        matches,
        truncated: next.is_some(),
        next_line: next.as_ref().map(|p| p.line),
        next_column: next.map(|p| p.column),
    })
}

fn text_replacements(
    source: &str,
    edits: Vec<FrontstageSourceTextEdit>,
) -> Result<Vec<SourceReplacement>> {
    if edits.is_empty() || edits.len() > MAX_SOURCE_EDITS {
        return Err(ControlPlaneError::InvalidInput("edits").into());
    }
    if edits
        .iter()
        .map(|edit| edit.old_text.len().saturating_add(edit.new_text.len()))
        .sum::<usize>()
        > MAX_TEXT_EDIT_BYTES
    {
        return Err(ControlPlaneError::InvalidInput("edits_size").into());
    }
    let starts = source_line_starts(source);
    edits
        .into_iter()
        .enumerate()
        .map(|(index, edit)| {
            if edit.old_text.is_empty() {
                return Err(FrontstageSourceEditError::edit("source_text_empty", index).into());
            }
            let offsets = matching_offsets(source, &edit.old_text, 0)
                .take(6)
                .collect::<Vec<_>>();
            if offsets.len() != 1 {
                let mut error = FrontstageSourceEditError::edit(
                    if offsets.is_empty() {
                        "source_text_not_found"
                    } else {
                        "source_text_ambiguous"
                    },
                    index,
                );
                error.candidates_truncated = offsets.len() > 5;
                error.candidates = offsets
                    .iter()
                    .take(5)
                    .map(|offset| position_at(source, &starts, *offset))
                    .collect();
                return Err(error.into());
            }
            Ok(SourceReplacement {
                index,
                start: offsets[0],
                end: offsets[0] + edit.old_text.len(),
                replacement: edit.new_text,
            })
        })
        .collect()
}

fn apply_replacements(
    source: &str,
    mut ranges: Vec<SourceReplacement>,
) -> Result<(String, Vec<FrontstageSourceChange>)> {
    if ranges
        .iter()
        .map(|range| range.replacement.len())
        .sum::<usize>()
        > MAX_TEXT_EDIT_BYTES
    {
        return Err(ControlPlaneError::InvalidInput("edits_size").into());
    }
    ranges.sort_by_key(|range| (range.start, range.end));
    for (i, range) in ranges.iter().enumerate() {
        if range.start > range.end {
            return Err(FrontstageSourceEditError::edit("source_edit_range", range.index).into());
        }
        if i > 0 && (range.start < ranges[i - 1].end || range.start == ranges[i - 1].start) {
            return Err(FrontstageSourceEditError::edit("source_edit_overlap", range.index).into());
        }
    }
    let starts = source_line_starts(source);
    let mut budget = MAX_DIFF_CHARS;
    let changes = ranges
        .iter()
        .map(|range| {
            let start = position_at(source, &starts, range.start);
            let end = position_at(source, &starts, range.end);
            let old = &source[range.start..range.end];
            let old_text = take_diff_text(old, &mut budget);
            let new_text = take_diff_text(&range.replacement, &mut budget);
            let truncated = old_text.len() < old.len() || new_text.len() < range.replacement.len();
            FrontstageSourceChange {
                edit_index: range.index,
                start_line: start.line,
                start_column: start.column,
                end_line: end.line,
                end_column: end.column,
                old_text,
                new_text,
                truncated,
            }
        })
        .collect();
    let mut patched = source.to_owned();
    for range in ranges.into_iter().rev() {
        patched.replace_range(range.start..range.end, &range.replacement);
    }
    Ok((patched, changes))
}

fn take_diff_text(text: &str, remaining: &mut usize) -> String {
    let result = text.chars().take(*remaining).collect::<String>();
    *remaining -= result.chars().count();
    result
}

#[cfg(test)]
#[path = "_tests/source_editing.rs"]
mod tests;
