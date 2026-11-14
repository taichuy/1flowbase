use super::*;
#[test]
fn event_output_reads_are_exact_repeatable_and_do_not_move_http_spool_cursors() {
    let stdout = "quoted \"text\"\nUnicode 文\n".repeat(10000);
    let stderr = "error\t\\trail\r\n".repeat(4000);
    let completed = CompletedCommand {
        metadata: ExecutionMetadata {
            exit_code: Some(7),
            signal: None,
            duration_ms: 123,
            timed_out: false,
            truncated: false,
            discarded_bytes: 0,
            encoding_lossy: false,
            output_complete: true,
            cleanup_complete: true,
        },
        stdout: spool::capture(
            std::io::Cursor::new(stdout.as_bytes()),
            Spool::new().unwrap(),
        )
        .unwrap(),
        stderr: spool::capture(
            std::io::Cursor::new(stderr.as_bytes()),
            Spool::new().unwrap(),
        )
        .unwrap(),
    };
    assert_eq!(
        completed.output_text().unwrap(),
        (stdout.clone(), stderr.clone())
    );
    assert_eq!(
        completed.output_text().unwrap(),
        (stdout.clone(), stderr.clone())
    );
    let mut http = Vec::new();
    for part in completed.into_json_parts().unwrap() {
        match part {
            ResponsePart::Bytes(bytes) => http.extend(bytes),
            ResponsePart::File(mut file) => {
                file.read_to_end(&mut http).unwrap();
            }
        }
    }
    let value: serde_json::Value = serde_json::from_slice(&http).unwrap();
    assert_eq!(value["data"]["stdout"], stdout);
    assert_eq!(value["data"]["stderr"], stderr);
    assert_eq!(value["data"]["exit_code"], 7);
}
