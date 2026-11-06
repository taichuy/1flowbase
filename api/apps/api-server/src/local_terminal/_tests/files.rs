use super::{dto::*, *};
use base64::Engine;
use std::{
    fs,
    path::{Path, PathBuf},
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("terminal-files-{}", uuid::Uuid::now_v7()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        path.to_str().unwrap().into()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn text_input(path: &str, offset: u64, max_bytes: usize) -> TextInput {
    TextInput {
        path: path.into(),
        timeout_ms: 1000,
        offset,
        max_bytes,
        expected_version: None,
    }
}
fn search_input(path: &Path) -> SearchInput {
    SearchInput {
        path: path.to_str().unwrap().into(),
        timeout_ms: 1000,
        pattern: "needle".into(),
        max_scan_bytes: 10000,
        max_matches: 100,
        max_depth: 10,
        max_line_bytes: 100,
    }
}
#[test]
fn terminal_text_preserves_utf8_offsets_and_exact_eof_and_rejects_stale_versions() {
    let temp = Temp::new();
    let path = temp.file("text", "a中b".as_bytes());
    let first = text(text_input(&path, 0, 3)).unwrap();
    assert_eq!(first.text, "a");
    assert_eq!(first.next_offset, 1);
    assert!(!first.eof);
    let second = text(text_input(&path, 1, 3)).unwrap();
    assert_eq!(second.text, "中");
    assert_eq!(second.next_offset, 4);
    assert!(text(text_input(&path, 2, 3)).is_err());
    assert!(text(text_input(&path, 1, 2)).is_err());
    let end = text(text_input(&path, 5, 1)).unwrap();
    assert!(end.eof);
    assert_eq!(end.text, "");
    assert!(text(text_input(&path, 6, 1)).is_err());
    fs::write(&path, b"changed-size").unwrap();
    let mut stale = text_input(&path, 0, 4);
    stale.expected_version = Some(first.version);
    assert!(matches!(text(stale), Err(FileError::Conflict(_))));
    temp.file("invalid", &[0xff]);
    assert!(text(text_input(temp.0.join("invalid").to_str().unwrap(), 0, 1)).is_err());
}
#[test]
fn terminal_range_returns_base64_and_empty_eof_without_padding_or_loss() {
    let temp = Temp::new();
    let path = temp.file("binary", &[0, 255, 3]);
    let read = |offset| {
        range(RangeInput {
            path: path.clone(),
            timeout_ms: 1000,
            offset,
            length: 9,
            expected_version: None,
        })
    };
    let output = read(1).unwrap();
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(output.data)
            .unwrap(),
        vec![255, 3]
    );
    assert_eq!(output.metadata.byte_count, 2);
    assert!(output.metadata.eof);
    let output = read(3).unwrap();
    assert!(output.data.is_empty());
    assert!(output.metadata.eof);
    assert!(read(4).is_err());
}
#[test]
fn terminal_lists_incremental_cursor_and_native_paths_and_rejects_invalid_budgets() {
    let temp = Temp::new();
    temp.file("a", b"one");
    temp.file("b", b"two");
    let page = |cursor| {
        list(ListInput {
            path: temp.0.to_str().unwrap().into(),
            timeout_ms: 1000,
            cursor,
            limit: 1,
        })
        .unwrap()
    };
    let a = page(0);
    let b = page(a.next_cursor);
    assert!(!a.eof);
    assert!(b.eof);
    assert_ne!(a.entries[0].name, b.entries[0].name);
    assert!(Path::new(&a.entries[0].path).is_absolute());
    assert_eq!(a.entries[0].kind, "file");
    assert!(a.entries[0].modified_at.is_some());
    #[cfg(unix)]
    assert!(a.entries[0].permissions.is_some());
    #[cfg(windows)]
    assert!(a.entries[0].permissions.is_none());
    assert!(path("relative/path").is_err());
    assert!(stat(PathInput {
        path: temp.0.to_str().unwrap().into(),
        timeout_ms: 0
    })
    .is_err());
    assert!(list(ListInput {
        path: temp.0.to_str().unwrap().into(),
        timeout_ms: 1000,
        cursor: 0,
        limit: 0
    })
    .is_err());
    #[cfg(windows)]
    {
        assert!(path("C:\\Windows").is_ok());
        assert!(path("C:relative").is_err());
    }
}
#[test]
fn terminal_search_accounts_budget_depth_long_lines_binary_and_byte_offsets() {
    let temp = Temp::new();
    let path = temp.file(
        "text",
        "中\nneedle here\nvery very long needle line\n".as_bytes(),
    );
    let mut input = search_input(Path::new(&path));
    input.max_line_bytes = 15;
    let output = search(input).unwrap();
    assert_eq!(output.matches.len(), 1);
    assert_eq!(output.matches[0].line_number, 2);
    assert_eq!(output.matches[0].byte_offset, 4);
    assert_eq!(output.skipped_long_lines, 1);
    let mut input = search_input(Path::new(&path));
    input.max_scan_bytes = 5;
    let output = search(input).unwrap();
    assert_eq!(output.scanned_bytes, 5);
    assert!(output.truncated);
    assert!(output.matches.is_empty());
    let binary = temp.file("binary", b"needle\n\0tail");
    let output = search(search_input(Path::new(&binary))).unwrap();
    assert!(output.matches.is_empty());
    assert_eq!(output.skipped_binary_files, 1);
    fs::create_dir(temp.0.join("nested")).unwrap();
    temp.file("nested/deep", b"needle");
    let mut input = search_input(&temp.0);
    input.max_depth = 0;
    let output = search(input).unwrap();
    assert!(output.depth_limited);
    assert!(output.matches.iter().all(|m| !m.path.ends_with("deep")));
    let mut input = search_input(&temp.0);
    input.pattern = "needle\n".into();
    assert!(search(input).is_err());
}
#[test]
fn terminal_media_uses_signatures_and_resource_links_do_not_load_content() {
    let temp = Temp::new();
    for (name, bytes, mime, image) in [
        ("png", b"\x89PNG\r\n\x1a\n".as_slice(), "image/png", true),
        ("jpg", b"\xff\xd8\xff", "image/jpeg", true),
        ("gif", b"GIF89a", "image/gif", true),
        ("webp", b"RIFF0000WEBP", "image/webp", true),
        ("wav", b"RIFF0000WAVE", "audio/wav", false),
        ("mp3", b"ID3", "audio/mpeg", false),
        ("ogg", b"OggS", "audio/ogg", false),
        ("flac", b"fLaC", "audio/flac", false),
    ] {
        let path = temp.file(name, bytes);
        let output = media(
            MediaInput {
                path,
                timeout_ms: 1000,
                max_file_bytes: 100,
            },
            image,
        )
        .unwrap();
        assert_eq!(output.mime_type, mime);
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(output.data)
                .unwrap(),
            bytes
        );
    }
    let path = temp.file("secret # 你好.png", b"not actually an image");
    assert!(media(
        MediaInput {
            path: path.clone(),
            timeout_ms: 1000,
            max_file_bytes: 100
        },
        true
    )
    .is_err());
    assert!(resource(ResourceInput {
        mime_type: None,
        path: path.clone(),
        timeout_ms: 1000,
        max_file_bytes: 1,
        link_only: false
    })
    .is_err());
    let link = resource(ResourceInput {
        mime_type: None,
        path: path.clone(),
        timeout_ms: 1000,
        max_file_bytes: 1,
        link_only: true,
    })
    .unwrap();
    assert!(link.data.is_none());
    assert_eq!(
        url::Url::parse(&link.uri).unwrap().to_file_path().unwrap(),
        Path::new(&path)
    );
    assert!(!link.uri.contains("actually"));
    let resource = resource(ResourceInput {
        mime_type: None,
        path,
        timeout_ms: 1000,
        max_file_bytes: 100,
        link_only: false,
    })
    .unwrap();
    assert_eq!(resource.mime_type, "application/octet-stream");
}
#[cfg(unix)]
#[test]
fn terminal_rejects_symlink_and_fifo_without_opening_blocking_streams() {
    use std::{ffi::CString, os::unix::fs::symlink};
    let temp = Temp::new();
    let source = temp.file("source", b"content");
    let link = temp.0.join("link");
    symlink(source, &link).unwrap();
    assert!(text(text_input(link.to_str().unwrap(), 0, 10)).is_err());
    let fifo = temp.0.join("fifo");
    let name = CString::new(fifo.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(range(RangeInput {
        path: fifo.to_str().unwrap().into(),
        timeout_ms: 100,
        offset: 0,
        length: 1,
        expected_version: None
    })
    .is_err());
    let output = search(search_input(&temp.0)).unwrap();
    assert_eq!(output.skipped_special_files, 2);
}

#[test]
fn terminal_resource_accepts_explicit_pdf_mime_and_rejects_header_injection() {
    let temp = Temp::new();
    let path = temp.file("document", b"%PDF-fixture");
    let read = |mime: &str| {
        resource(ResourceInput {
            path: path.clone(),
            timeout_ms: 1000,
            max_file_bytes: 100,
            link_only: false,
            mime_type: Some(mime.into()),
        })
    };
    assert_eq!(
        read("application/pdf").unwrap().mime_type,
        "application/pdf"
    );
    for mime in ["application/pdf\n", "text/", "/plain", "text/plain/extra"] {
        assert!(read(mime).is_err());
    }
}
