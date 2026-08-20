use plugin_framework::RuntimeTarget;

#[test]
fn runtime_target_parses_linux_musl_rust_triples() {
    let target = RuntimeTarget::from_rust_target_triple("aarch64-unknown-linux-musl").unwrap();

    assert_eq!(target.os, "linux");
    assert_eq!(target.arch, "arm64");
    assert_eq!(target.libc.as_deref(), Some("musl"));
    assert_eq!(target.asset_suffix(), "linux-arm64");
}

#[test]
fn runtime_target_parses_darwin_and_windows_triples() {
    let darwin = RuntimeTarget::from_rust_target_triple("aarch64-apple-darwin").unwrap();
    assert_eq!(darwin.os, "darwin");
    assert_eq!(darwin.arch, "arm64");
    assert_eq!(darwin.libc, None);
    assert_eq!(darwin.asset_suffix(), "darwin-arm64");
    assert_eq!(darwin.executable_suffix(), "");

    let windows = RuntimeTarget::from_rust_target_triple("x86_64-pc-windows-msvc").unwrap();
    assert_eq!(windows.os, "windows");
    assert_eq!(windows.arch, "amd64");
    assert_eq!(windows.libc.as_deref(), Some("msvc"));
    assert_eq!(windows.asset_suffix(), "windows-amd64");
    assert_eq!(windows.executable_suffix(), ".exe");
}

#[test]
fn runtime_target_builds_host_targets_from_os_and_arch_pairs() {
    let linux = RuntimeTarget::from_host_parts("linux", "x86_64").unwrap();
    assert_eq!(linux.rust_target_triple, "x86_64-unknown-linux-musl");

    let windows = RuntimeTarget::from_host_parts("windows", "aarch64").unwrap();
    assert_eq!(windows.rust_target_triple, "aarch64-pc-windows-msvc");
}

#[test]
fn runtime_target_detects_linux_elf_architecture() {
    let mut arm64_elf = vec![0_u8; 64];
    arm64_elf[..4].copy_from_slice(b"\x7fELF");
    arm64_elf[4] = 2;
    arm64_elf[5] = 1;
    arm64_elf[6] = 1;
    arm64_elf[18] = 0xb7;

    let target = RuntimeTarget::from_executable_bytes(&arm64_elf)
        .unwrap()
        .expect("an ELF runtime must expose its target platform");

    assert_eq!(target.os, "linux");
    assert_eq!(target.arch, "arm64");
}
