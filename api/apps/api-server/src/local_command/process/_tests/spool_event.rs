use super::*;
#[test]
fn positioned_event_reads_preserve_an_arbitrary_existing_http_cursor() {
    let expected = "quoted \"文\"\n".repeat(9000);
    let mut spool = capture(
        std::io::Cursor::new(expected.as_bytes()),
        Spool::new().unwrap(),
    )
    .unwrap();
    spool
        .file
        .as_mut()
        .unwrap()
        .seek(SeekFrom::Start(3))
        .unwrap();
    assert_eq!(spool.text().unwrap(), expected);
    assert_eq!(spool.file.as_mut().unwrap().stream_position().unwrap(), 3);
    assert_eq!(spool.text().unwrap(), expected);
    assert_eq!(spool.file.as_mut().unwrap().stream_position().unwrap(), 3);
}
#[test]
fn failed_event_decode_retains_file_ownership_position_and_original_bytes() {
    let mut spool = Spool::new().unwrap();
    spool
        .file
        .as_mut()
        .unwrap()
        .write_all(b"invalid\\q")
        .unwrap();
    let before = spool.file.as_mut().unwrap().stream_position().unwrap();
    assert!(spool.text().is_err());
    assert_eq!(
        spool.file.as_mut().unwrap().stream_position().unwrap(),
        before
    );
    let mut bytes = Vec::new();
    spool.into_file().unwrap().read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"invalid\\q");
}
