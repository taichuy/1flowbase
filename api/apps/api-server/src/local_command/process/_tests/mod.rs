use super::*;
use std::{io::Read, time::Duration};

fn body(command: &str) -> serde_json::Value {
    let result = execute(command, &Cancellation::default()).unwrap();
    decode(result)
}
fn decode(result: CompletedCommand) -> serde_json::Value {
    let mut bytes = Vec::new();
    for part in result.into_json_parts().unwrap() {
        match part {
            ResponsePart::Bytes(part) => bytes.extend(part),
            ResponsePart::File(mut file) => {
                file.read_to_end(&mut bytes).unwrap();
            }
        }
    }
    serde_json::from_slice(&bytes).unwrap()
}
fn shell<'a>(unix: &'a str, windows: &'a str) -> &'a str {
    if cfg!(windows) {
        windows
    } else {
        unix
    }
}

#[test]
fn local_command_native_shell_stdout_stderr_and_exit() {
    let value = body(shell(
        "printf hello; printf problem >&2; exit 7",
        "<nul set /p =hello& <nul set /p =problem>&2& exit /b 7",
    ));
    assert_eq!(value["data"]["stdout"], "hello");
    assert_eq!(value["data"]["stderr"], "problem");
    assert_eq!(value["data"]["exit_code"], 7);
    assert_eq!(value["data"]["cleanup_complete"], true);
    assert_eq!(value["data"]["truncated"], false);
}

#[test]
fn local_command_native_quotes_unicode_and_pipeline() {
    let value = body(shell(
        "printf '你好 世界' | cat",
        "echo 你好 世界|findstr .",
    ));
    assert_eq!(
        value["data"]["stdout"].as_str().unwrap().trim(),
        "你好 世界"
    );
    assert_eq!(value["data"]["encoding_lossy"], false);
    let path = std::env::temp_dir().join(format!("command path 中文 {}", uuid::Uuid::now_v7()));
    std::fs::create_dir(&path).unwrap();
    let command = if cfg!(windows) {
        format!("cd /d \"{}\" && cd", path.display())
    } else {
        format!("cd '{}' && pwd", path.display())
    };
    let value = body(&command);
    assert_eq!(value["data"]["exit_code"], 0, "{value}");
    assert!(
        value["data"]["stdout"]
            .as_str()
            .unwrap()
            .contains("command path 中文"),
        "{value}"
    );
    std::fs::remove_dir(path).unwrap();
}

#[test]
fn local_command_native_calls_do_not_share_environment() {
    body(shell(
        "export ONEFLOWBASE_COMMAND_FIXTURE=yes",
        "set ONEFLOWBASE_COMMAND_FIXTURE=yes",
    ));
    let value = body(shell(
        "printf %s \"${ONEFLOWBASE_COMMAND_FIXTURE-unset}\"",
        "if not defined ONEFLOWBASE_COMMAND_FIXTURE echo unset",
    ));
    assert_eq!(value["data"]["stdout"].as_str().unwrap().trim(), "unset");
}

#[test]
fn local_command_native_complete_output_is_spooled_not_truncated() {
    let value = body(shell(
        "(head -c 2097152 /dev/zero >&2) & head -c 2097152 /dev/zero; wait",
        "powershell.exe -NoProfile -NonInteractive -Command \"$s='x'*2097152; [Console]::Out.Write($s); [Console]::Error.Write($s)\"",
    ));
    assert_eq!(value["data"]["stdout"].as_str().unwrap().len(), 2097152);
    assert_eq!(value["data"]["stderr"].as_str().unwrap().len(), 2097152);
    assert_eq!(value["data"]["discarded_bytes"], 0);
    assert_eq!(value["data"]["truncated"], false);
    assert_eq!(value["data"]["output_complete"], true);
}

#[test]
fn local_command_native_cancellation_reaps_owned_process() {
    let cancellation = Cancellation::default();
    let token = cancellation.clone();
    let task = std::thread::spawn(move || {
        execute(shell("exec sleep 30", "ping -n 30 127.0.0.1 >nul"), &token)
    });
    std::thread::sleep(Duration::from_millis(150));
    cancellation.cancel();
    assert_eq!(
        task.join().unwrap().err().unwrap().kind(),
        io::ErrorKind::Interrupted
    );
}

#[test]
fn local_command_native_background_child_is_terminated() {
    let directory = std::env::temp_dir().join(format!("command-child-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir(&directory).unwrap();
    let ready = directory.join("ready");
    let marker = directory.join("survived");
    let command = if cfg!(windows) {
        let child = directory.join("child.ps1");
        let parent = directory.join("parent.ps1");
        let quote = |p: &std::path::Path| p.display().to_string().replace('\'', "''");
        let child_script = format!("Set-Content -LiteralPath '{}' -Value $PID; Start-Sleep -Seconds 3; Set-Content -LiteralPath '{}' -Value survived", quote(&ready), quote(&marker));
        let parent_script = format!(
            r#"$a = '-NoProfile -NonInteractive -File "{0}"'; Start-Process powershell.exe -ArgumentList $a -NoNewWindow; $end = (Get-Date).AddSeconds(20); while (-not (Test-Path -LiteralPath '{1}')) {{ if ((Get-Date) -gt $end) {{ throw 'child did not start' }}; Start-Sleep -Milliseconds 10 }}; Write-Output ready"#,
            quote(&child),
            quote(&ready)
        );
        // Windows PowerShell 5 reads a BOM to recognize UTF-8 script paths/content.
        std::fs::write(&child, format!("\u{feff}{child_script}")).unwrap();
        std::fs::write(&parent, format!("\u{feff}{parent_script}")).unwrap();
        format!(
            "powershell.exe -NoProfile -NonInteractive -File \"{}\"",
            parent.display()
        )
    } else {
        format!("(echo started > '{}'; sleep 3; echo survived > '{}') & while [ ! -f '{}' ]; do sleep 0.01; done; printf ready", ready.display(), marker.display(), ready.display())
    };
    let value = body(&command);
    assert_eq!(value["data"]["exit_code"], 0);
    assert!(
        ready.exists(),
        "fixture must prove the background child actually started"
    );
    assert!(value["data"]["stdout"].as_str().unwrap().contains("ready"));
    std::thread::sleep(Duration::from_secs(4));
    assert!(
        !marker.exists(),
        "background child survived the command boundary"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn local_command_spool_handles_split_utf8_json_escaping_and_invalid_bytes() {
    struct OneByte(std::io::Cursor<Vec<u8>>);
    impl Read for OneByte {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.0.read(&mut bytes[..1])
        }
    }
    let mut bytes = "你好\"\\\n\u{0}".as_bytes().to_vec();
    bytes.extend_from_slice(&[0xff, 0xe4]);
    let spool = spool::capture(
        OneByte(std::io::Cursor::new(bytes.clone())),
        Spool::new().unwrap(),
    )
    .unwrap();
    assert!(spool.encoding_lossy);
    let mut json = String::new();
    spool
        .into_file()
        .unwrap()
        .read_to_string(&mut json)
        .unwrap();
    let decoded: String = serde_json::from_str(&format!("\"{json}\"")).unwrap();
    assert_eq!(decoded, String::from_utf8_lossy(&bytes));
}

#[test]
fn local_command_spool_carrier_is_private_and_deleted_on_close() {
    let spool = Spool::new().unwrap();
    let file = spool.into_file().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = file.metadata().unwrap();
        assert_eq!(metadata.nlink(), 0);
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    }
    #[cfg(windows)]
    {
        use std::os::windows::{ffi::OsStringExt, io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;
        let size =
            unsafe { GetFinalPathNameByHandleW(file.as_raw_handle(), std::ptr::null_mut(), 0, 0) };
        assert!(size > 0);
        let mut buffer = vec![0u16; size as usize + 1];
        let length = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                0,
            )
        };
        assert!(length > 0 && length < buffer.len() as u32);
        let path =
            std::path::PathBuf::from(std::ffi::OsString::from_wide(&buffer[..length as usize]));
        assert!(path.exists());
        drop(file);
        assert!(
            !path.exists(),
            "delete-on-close must actually remove the carrier"
        );
    }
    #[cfg(unix)]
    drop(file);
}

#[cfg(windows)]
#[test]
fn local_command_windows_payload_is_interpreted_once() {
    let value = body("echo literal! & echo ^&^|^>^< & echo %COMSPEC%");
    let output = value["data"]["stdout"].as_str().unwrap();
    let lines: Vec<_> = output.lines().map(str::trim).collect();
    assert_eq!(lines[0], "literal!", "{value}");
    assert_eq!(lines[1], "&|><", "{value}");
    assert!(
        lines[2].to_ascii_lowercase().ends_with("cmd.exe"),
        "{value}"
    );
    let unicode = body("echo 你好 世界");
    assert_eq!(
        unicode["data"]["stdout"].as_str().unwrap().trim(),
        "你好 世界",
        "{unicode}"
    );
}
