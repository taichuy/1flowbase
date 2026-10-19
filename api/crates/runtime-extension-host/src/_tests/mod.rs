mod managed_event_transport;
mod managed_hook_transport;
mod network_egress_runtime;
mod provider_stdio_streaming;
mod stdio_host_calls;

mod managed_capability_multiplex;

#[cfg(unix)]
pub(crate) fn write_executable_fixture(path: &std::path::Path, bytes: &[u8], mode: u32) {
    use std::{fs, io::Write, os::unix::fs::PermissionsExt};

    let mut writer = fs::File::create(path).unwrap();
    writer.write_all(bytes).unwrap();
    writer
        .set_permissions(fs::Permissions::from_mode(mode))
        .unwrap();
    // Concurrent fork can inherit the writable descriptor until exec, even
    // after this thread closes it. The shared lock waits for those copies too.
    // See rust-lang/rust#114554; keep this fixture barrier outside runtime code.
    writer.lock().unwrap();
    drop(writer);
    let reader = fs::File::open(path).unwrap();
    reader.lock_shared().unwrap();
    drop(reader);
}
