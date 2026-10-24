use super::*;

#[test]
fn streaming_answer_preserves_nul_literal_escapes_and_field_presence() {
    for (body, expected) in [
        (
            r#"{"answer":"real\u0000nul","text":"fallback"}"#,
            Some("real\0nul"),
        ),
        (r#"{"answer":"literal\\u0000"}"#, Some("literal\\u0000")),
        (r#"{"answer":null,"text":"must not win"}"#, None),
        (
            r#"{"text":"first","answer":[],"output":"must not win"}"#,
            None,
        ),
        (r#"{"output":"third","text":"second"}"#, Some("second")),
        (r#"{"answer":"first","answer":"last"}"#, Some("last")),
        (r#"["ignored"]"#, None),
    ] {
        assert_eq!(parse(body.as_bytes()).unwrap().as_deref(), expected);
    }
    assert!(parse(br#"{"answer":"complete", "truncated":"# as &[u8]).is_err());
}

#[test]
fn streaming_answer_ignores_large_context_without_materializing_it() {
    let input = Cursor::new(br#"{"context":""#)
        .chain(io::repeat(b'x').take(8 * 1024 * 1024))
        .chain(Cursor::new(
            br#"","answer":"needed\u0000scalar","output":[{"ignored":"body"}]}"#,
        ));
    assert_eq!(parse(input).unwrap().as_deref(), Some("needed\0scalar"));
}

#[tokio::test]
async fn streaming_answer_handles_chunks_split_inside_unicode_escape() {
    let (sender, receiver) = mpsc::channel(2);
    let parser = tokio::task::spawn_blocking(move || {
        parse(ChunkReader {
            receiver,
            current: Cursor::new(Vec::new()),
        })
    });
    for chunk in [br#"{"answer":"a\u0"# as &[u8], br#"000b"}"#] {
        sender.send(chunk.to_vec()).await.unwrap();
    }
    drop(sender);
    assert_eq!(parser.await.unwrap().unwrap().as_deref(), Some("a\0b"));
}

#[tokio::test]
async fn streaming_answer_producer_close_releases_blocking_reader() {
    let (sender, receiver) = mpsc::channel(2);
    let parser = tokio::task::spawn_blocking(move || {
        parse(ChunkReader {
            receiver,
            current: Cursor::new(Vec::new()),
        })
    });
    sender
        .send(br#"{"answer":"unfinished"#.to_vec())
        .await
        .unwrap();
    drop(sender);
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(5), parser)
            .await
            .expect("producer close must release the blocking reader")
            .unwrap()
            .is_err()
    );
}
