use super::FileError;
use super::{dto::*, files, invalid, path, Budget};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
};

pub(crate) fn search(input: SearchInput) -> Result<SearchOutput, FileError> {
    let budget = Budget::new(input.timeout_ms)?;
    let root = path(&input.path)?;
    if input.pattern.is_empty()
        || input.pattern.contains(['\n', '\r', '\0'])
        || input.max_scan_bytes == 0
        || input.max_matches == 0
        || input.max_line_bytes == 0
    {
        return Err(invalid("terminal_search_input_invalid"));
    }
    let mut out = SearchOutput::default();
    let meta = fs::symlink_metadata(&root)?;
    if meta.is_file() {
        scan(&root, &input, &budget, &mut out)?;
        return Ok(out);
    }
    if !meta.is_dir() {
        return Err(invalid("terminal_search_path_invalid"));
    }
    // One lazy iterator per depth; directory breadth never becomes a path allocation budget.
    let mut stack = vec![(fs::read_dir(&root)?, 0_usize)];
    while let Some((entries, depth)) = stack.last_mut() {
        budget.check()?;
        if out.scanned_bytes >= input.max_scan_bytes || out.matches.len() >= input.max_matches {
            out.truncated = true;
            break;
        }
        let Some(entry) = entries.next().transpose()? else {
            stack.pop();
            continue;
        };
        let child_depth = *depth + 1;
        let meta = fs::symlink_metadata(entry.path())?;
        if meta.file_type().is_symlink() || (!meta.is_dir() && !meta.is_file()) {
            out.skipped_special_files += 1;
            continue;
        }
        if meta.is_dir() {
            if child_depth > input.max_depth {
                out.depth_limited = true;
                continue;
            }
            stack.push((fs::read_dir(entry.path())?, child_depth));
        } else {
            scan(&entry.path(), &input, &budget, &mut out)?;
        }
    }
    Ok(out)
}
fn scan(
    path: &Path,
    input: &SearchInput,
    budget: &Budget,
    out: &mut SearchOutput,
) -> Result<(), FileError> {
    let (file, _) = files::open(path)?;
    let mut reader = BufReader::with_capacity(8192, file);
    let mut line = Vec::new();
    let mut long = false;
    let mut line_number = 1;
    let mut offset = 0;
    let mut line_offset = 0;
    let before_matches = out.matches.len();
    loop {
        budget.check()?;
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if long {
                out.skipped_long_lines += 1;
            } else if !line.is_empty() {
                if std::str::from_utf8(&line).is_err() {
                    out.skipped_binary_files += 1;
                    out.matches.truncate(before_matches);
                } else {
                    match_line(path, input, out, &line, line_number, line_offset)?;
                }
            }
            return Ok(());
        }
        if out.scanned_bytes >= input.max_scan_bytes || out.matches.len() >= input.max_matches {
            out.truncated = true;
            return Ok(());
        }
        let allowed = available
            .len()
            .min((input.max_scan_bytes - out.scanned_bytes).min(usize::MAX as u64) as usize);
        let mut used = 0;
        for &byte in &available[..allowed] {
            used += 1;
            out.scanned_bytes += 1;
            offset += 1;
            if byte == 0 {
                out.skipped_binary_files += 1;
                out.matches.truncate(before_matches);
                return Ok(());
            }
            if byte == b'\n' {
                if long {
                    out.skipped_long_lines += 1;
                } else if std::str::from_utf8(&line).is_err() {
                    out.skipped_binary_files += 1;
                    out.matches.truncate(before_matches);
                    return Ok(());
                } else {
                    match_line(path, input, out, &line, line_number, line_offset)?;
                }
                line.clear();
                long = false;
                line_number += 1;
                line_offset = offset;
                if out.matches.len() >= input.max_matches {
                    out.truncated = true;
                    reader.consume(used);
                    return Ok(());
                }
            } else if !long {
                if line.len() == input.max_line_bytes {
                    line.clear();
                    long = true;
                } else {
                    line.push(byte);
                }
            }
        }
        reader.consume(used);
    }
}
fn match_line(
    path: &Path,
    input: &SearchInput,
    out: &mut SearchOutput,
    line: &[u8],
    line_number: u64,
    byte_offset: u64,
) -> Result<(), FileError> {
    let Ok(text) = std::str::from_utf8(line) else {
        out.skipped_binary_files += 1;
        return Ok(());
    };
    if text.contains(&input.pattern) {
        out.matches.push(SearchMatch {
            path: path.to_string_lossy().into_owned(),
            line_number,
            byte_offset,
            text: text.trim_end_matches('\r').into(),
        });
    }
    Ok(())
}
