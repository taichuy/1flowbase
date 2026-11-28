pub(crate) fn write_executable_fixture(path: &std::path::Path, bytes: &[u8], mode: u32) {
    use std::{fs, io::Write, os::unix::fs::PermissionsExt};

    // The package may already have a placeholder at `path`. Its independently opened
    // writers do not participate in the lock below; never exec that same inode.
    let staged = path.with_added_extension("publishing");
    let mut writer = fs::File::create_new(&staged).unwrap();
    writer.write_all(bytes).unwrap();
    writer
        .set_permissions(fs::Permissions::from_mode(mode))
        .unwrap();
    // Concurrent fork can inherit the writable descriptor until exec, even
    // after this thread closes it. The shared lock waits for those copies too.
    // See rust-lang/rust#114554; keep this fixture barrier outside runtime code.
    writer.lock().unwrap();
    drop(writer);
    let reader = fs::File::open(&staged).unwrap();
    reader.lock_shared().unwrap();
    fs::rename(staged, path).unwrap();
}

#[test]
fn replacement_executes_while_a_writer_still_holds_the_placeholder() {
    use std::{fs, process::Command};

    let dir = std::env::temp_dir().join(format!("fixture-publication-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("worker");
    // Same kernel state as another test's fork inheriting the original writable fd.
    let placeholder = fs::File::create_new(&path).unwrap();
    write_executable_fixture(&path, b"#!/bin/sh\nprintf replacement\n", 0o700);
    let output = Command::new(&path).output();
    drop(placeholder);
    fs::remove_dir_all(dir).unwrap();

    let output = output.expect("spawn replacement while predecessor is still open for writing");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"replacement");
}
