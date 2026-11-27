use super::*;

#[test]
fn payload_digest_matches_sorted_nul_delimited_reference() {
    let root = std::env::temp_dir().join(format!("plugin-payload-{}", Uuid::now_v7()));
    for dir in ["_meta", "a/deep"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    // "a-file" sorts before "a/deep/empty", even though the directory is visited first.
    let files = [
        ("a-file", vec![0, 1, 0, 2]),
        ("a/deep/empty", vec![]),
        (
            "a/large",
            (0..200 * 1024).map(|i| (i % 251) as u8).collect(),
        ),
    ];
    let mut reference = Vec::new();
    for (path, bytes) in files {
        fs::write(root.join(path), &bytes).unwrap();
        reference.extend_from_slice(path.as_bytes());
        reference.push(0);
        reference.extend(bytes);
        reference.push(0);
    }
    fs::write(root.join("_meta/ignored"), b"not in payload").unwrap();
    let digest = payload_sha256(&root);
    fs::remove_dir_all(root).unwrap();
    assert_eq!(digest.unwrap(), sha256_hex(&reference));
}

#[test]
#[cfg(target_os = "linux")]
fn package_runtime_target_checks_only_the_elf_header() {
    use std::io::Write;

    let root = std::env::temp_dir().join(format!("package-elf-header-{}", Uuid::now_v7()));
    fs::create_dir_all(&root).unwrap();
    let mut manifest = parse_plugin_manifest(include_str!(
        "../../../../plugins/fixtures/acme.composition-a/manifest.yaml"
    ))
    .unwrap();
    manifest.runtime.entry = "runtime".into();
    let path = root.join(&manifest.runtime.entry);
    let host = RuntimeTarget::current_host().unwrap();
    let machine: u16 = if host.arch == "amd64" { 0x3e } else { 0xb7 };
    let mut header = [0; 20];
    header[..4].copy_from_slice(b"\x7fELF");
    header[5] = 1;
    header[18..].copy_from_slice(&machine.to_le_bytes());
    let mut file = fs::File::create(&path).unwrap();
    file.write_all(&header).unwrap();
    file.set_len(8 * 1024 * 1024).unwrap();
    assert!(ensure_package_runtime_target_compatible(&root, &manifest).is_ok());

    header[18..].copy_from_slice(&(if machine == 0x3e { 0xb7u16 } else { 0x3e }).to_le_bytes());
    fs::write(&path, header).unwrap();
    assert!(matches!(
        ensure_package_runtime_target_compatible(&root, &manifest),
        Err(PluginFrameworkError::PackageRuntimeTargetMismatch { .. })
    ));

    fs::write(&path, &header[..19]).unwrap();
    assert!(
        matches!(ensure_package_runtime_target_compatible(&root, &manifest), Err(PluginFrameworkError::InvalidProviderPackage { message }) if message == "ELF runtime header is incomplete")
    );
    fs::write(&path, b"#!/bin/sh\n").unwrap();
    assert!(ensure_package_runtime_target_compatible(&root, &manifest).is_ok());
    fs::remove_dir_all(root).unwrap();
}
