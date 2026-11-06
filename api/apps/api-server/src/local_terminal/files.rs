use super::FileError;
use super::{dto::*, invalid, path, Budget};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::UNIX_EPOCH,
};

pub(super) fn metadata(path: &Path, meta: &Metadata) -> FileMetadata {
    let modified = meta
        .modified()
        .ok()
        .and_then(|v| v.duration_since(UNIX_EPOCH).ok());
    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        format!("{:o}", meta.permissions().mode() & 0o7777)
    };
    #[cfg(not(unix))]
    let permissions = if meta.permissions().readonly() {
        "readonly"
    } else {
        "readwrite"
    }
    .to_owned();
    FileMetadata {
        name: path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned(),
        path: path.to_string_lossy().into_owned(),
        kind: if meta.is_file() {
            "file"
        } else if meta.is_dir() {
            "directory"
        } else if meta.file_type().is_symlink() {
            "symlink"
        } else {
            "special"
        }
        .into(),
        size: meta.len(),
        modified_at: modified
            .and_then(|v| {
                time::OffsetDateTime::from_unix_timestamp_nanos(v.as_nanos() as i128).ok()
            })
            .and_then(|v| {
                v.format(&time::format_description::well_known::Rfc3339)
                    .ok()
            }),
        permissions,
        version: format!(
            "weak:{}:{}",
            meta.len(),
            modified.map(|v| v.as_nanos()).unwrap_or_default()
        ),
    }
}
/// Nonblocking descriptor open plus descriptor metadata rejects FIFO/device/symlink races.
pub(super) fn open(path: &Path) -> Result<(File, FileMetadata), FileError> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() || before.file_type().is_symlink() {
        return Err(invalid("terminal_regular_file_required"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000);
    }
    let file = options.open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(invalid("terminal_regular_file_required"));
    }
    Ok((file, metadata(path, &meta)))
}
fn unchanged(file: &File, path: &Path, version: &str) -> Result<(), FileError> {
    if metadata(path, &file.metadata()?).version != version {
        return Err(FileError::Conflict("terminal_file_changed").into());
    }
    Ok(())
}
pub(crate) fn stat(input: PathInput) -> Result<FileMetadata, FileError> {
    let budget = Budget::new(input.timeout_ms)?;
    let path = path(&input.path)?;
    let value = metadata(&path, &fs::symlink_metadata(&path)?);
    budget.check()?;
    Ok(value)
}
pub(crate) fn list(input: ListInput) -> Result<ListOutput, FileError> {
    let budget = Budget::new(input.timeout_ms)?;
    let path = path(&input.path)?;
    if input.limit == 0 {
        return Err(invalid("terminal_limit_invalid"));
    }
    let mut iter = fs::read_dir(&path)?;
    for _ in 0..input.cursor {
        budget.check()?;
        if iter.next().transpose()?.is_none() {
            return Err(invalid("terminal_cursor_invalid"));
        }
    }
    let mut entries = Vec::new();
    for _ in 0..input.limit {
        budget.check()?;
        let Some(entry) = iter.next().transpose()? else {
            break;
        };
        entries.push(metadata(
            &entry.path(),
            &fs::symlink_metadata(entry.path())?,
        ));
    }
    let eof = iter.next().transpose()?.is_none();
    budget.check()?;
    Ok(ListOutput {
        path: input.path,
        next_cursor: input
            .cursor
            .checked_add(entries.len() as u64)
            .ok_or_else(|| invalid("terminal_cursor_invalid"))?,
        entries,
        eof,
    })
}
fn read_chunk(
    path_value: &str,
    offset: u64,
    length: usize,
    expected: Option<&str>,
    budget: &Budget,
) -> Result<(Vec<u8>, FileMetadata), FileError> {
    if length == 0 {
        return Err(invalid("terminal_read_budget_invalid"));
    }
    let path = path(path_value)?;
    let (mut file, meta) = open(&path)?;
    if expected.is_some_and(|v| v != meta.version) {
        return Err(FileError::Conflict("terminal_file_changed").into());
    }
    if offset > meta.size {
        return Err(invalid("terminal_offset_invalid"));
    }
    let length = (meta.size - offset).min(length as u64);
    file.seek(SeekFrom::Start(offset))?;
    let mut data = Vec::new();
    let mut remaining = length;
    let mut chunk = [0_u8; 65536];
    while remaining > 0 {
        budget.check()?;
        let cap = remaining.min(chunk.len() as u64) as usize;
        let n = file.read(&mut chunk[..cap])?;
        if n == 0 {
            return Err(invalid("terminal_file_changed"));
        }
        data.extend_from_slice(&chunk[..n]);
        remaining -= n as u64;
    }
    unchanged(&file, &path, &meta.version)?;
    budget.check()?;
    Ok((data, meta))
}
pub(crate) fn text(input: TextInput) -> Result<TextOutput, FileError> {
    let budget = Budget::new(input.timeout_ms)?;
    let (mut data, meta) = read_chunk(
        &input.path,
        input.offset,
        input.max_bytes,
        input.expected_version.as_deref(),
        &budget,
    )?;
    match std::str::from_utf8(&data) {
        Ok(_) => {}
        Err(error)
            if error.error_len().is_none() && input.offset + (data.len() as u64) < meta.size =>
        {
            data.truncate(error.valid_up_to());
            if data.is_empty() {
                return Err(invalid("terminal_utf8_budget_too_small"));
            }
        }
        Err(_) => return Err(invalid("terminal_invalid_utf8")),
    }
    let next_offset = input.offset + data.len() as u64;
    Ok(TextOutput {
        path: input.path,
        offset: input.offset,
        text: String::from_utf8(data).map_err(|_| invalid("terminal_invalid_utf8"))?,
        next_offset,
        eof: next_offset == meta.size,
        version: meta.version,
    })
}
pub(crate) fn range(input: RangeInput) -> Result<RangeOutput, FileError> {
    let budget = Budget::new(input.timeout_ms)?;
    let (data, meta) = read_chunk(
        &input.path,
        input.offset,
        input.length,
        input.expected_version.as_deref(),
        &budget,
    )?;
    let next_offset = input.offset + data.len() as u64;
    Ok(RangeOutput {
        data: STANDARD.encode(&data),
        mime_type: "application/octet-stream".into(),
        metadata: RangeMetadata {
            path: input.path,
            offset: input.offset,
            byte_count: data.len(),
            next_offset,
            eof: next_offset == meta.size,
            version: meta.version,
        },
    })
}
pub(super) fn mime(data: &[u8]) -> &'static str {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if data.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        "image/gif"
    } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WAVE") {
        "audio/wav"
    } else if data.starts_with(b"ID3")
        || (data.len() >= 2 && data[0] == 0xff && data[1] & 0xe0 == 0xe0)
    {
        "audio/mpeg"
    } else if data.starts_with(b"OggS") {
        "audio/ogg"
    } else if data.starts_with(b"fLaC") {
        "audio/flac"
    } else {
        "application/octet-stream"
    }
}
fn all(input: &MediaInput) -> Result<(Vec<u8>, FileMetadata), FileError> {
    let budget = Budget::new(input.timeout_ms)?;
    if input.max_file_bytes == 0 {
        return Err(invalid("terminal_file_budget_invalid"));
    }
    let path = path(&input.path)?;
    let (_, meta) = open(&path)?;
    if meta.size > input.max_file_bytes {
        return Err(invalid("terminal_file_budget_exceeded"));
    }
    let length =
        usize::try_from(meta.size.max(1)).map_err(|_| invalid("terminal_file_budget_exceeded"))?;
    read_chunk(&input.path, 0, length, Some(&meta.version), &budget)
}
pub(crate) fn media(input: MediaInput, image: bool) -> Result<MediaOutput, FileError> {
    let (data, meta) = all(&input)?;
    let kind = mime(&data);
    if !(if image {
        kind.starts_with("image/")
    } else {
        kind.starts_with("audio/")
    }) {
        return Err(invalid("terminal_media_signature_invalid"));
    }
    Ok(MediaOutput {
        data: STANDARD.encode(data),
        mime_type: kind.into(),
        metadata: meta,
    })
}
pub(crate) fn resource(input: ResourceInput) -> Result<ResourceOutput, FileError> {
    let native_path = path(&input.path)?;
    let mime_type = input
        .mime_type
        .unwrap_or_else(|| "application/octet-stream".into());
    // RFC 6838 type/subtype token form; parameters and control characters are not accepted.
    let valid = mime_type.split_once('/').is_some_and(|(kind, subtype)| {
        [kind, subtype].iter().all(|part| {
            !part.is_empty()
                && part.len() <= 127
                && part.as_bytes()[0].is_ascii_alphanumeric()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&byte))
        })
    });
    if !valid {
        return Err(invalid("terminal_mime_type_invalid"));
    }
    let uri = url::Url::from_file_path(&native_path)
        .map_err(|_| invalid("terminal_path_uri_invalid"))?
        .to_string();
    let (data, meta) = if input.link_only {
        let meta = stat(PathInput {
            path: input.path,
            timeout_ms: input.timeout_ms,
        })?;
        if meta.kind != "file" || input.max_file_bytes == 0 {
            return Err(invalid("terminal_regular_file_required"));
        }
        (None, meta)
    } else {
        let (data, meta) = all(&MediaInput {
            path: input.path,
            timeout_ms: input.timeout_ms,
            max_file_bytes: input.max_file_bytes,
        })?;
        (Some(STANDARD.encode(data)), meta)
    };
    Ok(ResourceOutput {
        uri,
        name: meta.name.clone(),
        data,
        mime_type,
        metadata: meta,
    })
}
