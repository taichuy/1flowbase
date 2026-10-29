use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};

fn release(version: &str, prerelease: bool, asset: &str) -> Release {
    serde_json::from_value(serde_json::json!({"tag_name":format!("v{version}"),"draft":false,"prerelease":prerelease,"assets":[{"name":asset,"browser_download_url":"https://github.com/taichuy/1flowbase/releases/download/fixture/package","size":1,"digest":"sha256:00"}]})).unwrap()
}
#[test]
fn release_selection_respects_version_platform_and_channel() {
    let f = super::super::tests::Fixture::new();
    let mut old = Manifest::read(&f.old).unwrap();
    old.version = "1.0.0".into();
    let name = format!(
        "1flowbase-{}-{}.{}",
        old.platform,
        old.arch,
        if old.platform == "windows" {
            "zip"
        } else {
            "tar.gz"
        }
    );
    let selected = select(
        vec![
            release("0.9.0", false, &name),
            release("1.1.0", false, &name),
            release("2.0.0-beta.1", true, &name),
            release("9.0.0", false, "other-platform.zip"),
        ],
        &old,
    )
    .unwrap()
    .unwrap();
    assert_eq!(selected.version.to_string(), "1.1.0");
    old.version = "1.1.0-native.1".into();
    assert_eq!(
        select(vec![release("1.1.0-native.2", true, &name)], &old)
            .unwrap()
            .unwrap()
            .version
            .to_string(),
        "1.1.0-native.2"
    );
    assert!(select(vec![release("1.1.0-native.1", true, &name)], &old)
        .unwrap()
        .is_none());
    assert!(!valid_download_url(
        "https://example.com/malware",
        "https://api.github.com"
    ));
}
struct Server {
    url: String,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn server(name: &str, bytes: Vec<u8>, digest: String, version: &str) -> Server {
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    socket.set_nonblocking(true).unwrap();
    let url = format!("http://{}", socket.local_addr().unwrap());
    let metadata=serde_json::to_vec(&serde_json::json!([{"tag_name":format!("v{version}"),"draft":false,"prerelease":!semver::Version::parse(version).unwrap().pre.is_empty(),"assets":[{"name":name,"browser_download_url":format!("{url}/archive"),"size":bytes.len(),"digest":format!("sha256:{digest}")}]}])).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let running = stop.clone();
    let thread = thread::spawn(move || {
        while !running.load(Ordering::SeqCst) {
            match socket.accept() {
                Ok((mut stream, _)) => {
                    // BSD/macOS inherits O_NONBLOCK from the listening socket; Linux does not.
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_write_timeout(Some(std::time::Duration::from_secs(60)))
                        .unwrap();
                    stream
                        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                        .unwrap();
                    let mut request = Vec::new();
                    let mut buffer = [0; 1024];
                    while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                        let n = stream.read(&mut buffer).unwrap();
                        assert!(
                            n > 0 && request.len() + n <= 8192,
                            "Invalid fixture HTTP request"
                        );
                        request.extend_from_slice(&buffer[..n]);
                    }
                    let body = if String::from_utf8_lossy(&request).starts_with("GET /archive ") {
                        &bytes
                    } else {
                        &metadata
                    };
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .unwrap();
                    stream.write_all(body).unwrap();
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(std::time::Duration::from_millis(5))
                }
                Err(error) => panic!("{error}"),
            }
        }
    });
    Server {
        url,
        stop,
        thread: Some(thread),
    }
}
#[test]
fn downloads_real_archive_checks_digest_and_leaves_existing_installation_untouched() {
    use sha2::{Digest, Sha256};
    let f = super::super::tests::Fixture::new();
    let old = Manifest::read(&f.old).unwrap();
    let name = format!("1flowbase-{}-{}", old.platform, old.arch);
    let bytes = if cfg!(windows) {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for file in [
            Manifest::executable(),
            ".env.example",
            "web/index.html",
            super::super::manifest::NAME,
        ] {
            zip.start_file(
                format!("{name}/{file}"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(&fs::read(f.new.join(file)).unwrap()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    } else {
        let gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut tar = tar::Builder::new(gzip);
        tar.append_dir_all(&name, &f.new).unwrap();
        tar.into_inner().unwrap().finish().unwrap()
    };
    let asset = format!("{name}.{}", if cfg!(windows) { "zip" } else { "tar.gz" });
    let digest = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let server = server(&asset, bytes.clone(), digest, "1.1.0");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    let work = f.directory.join("download");
    let candidate = runtime
        .block_on(download(&client, &old, &work, &server.url))
        .unwrap()
        .unwrap();
    Manifest::read(&candidate)
        .unwrap()
        .verify(&candidate)
        .unwrap();
    assert_eq!(
        fs::read_to_string(f.old.join(Manifest::executable())).unwrap(),
        "old"
    );
    let corrupt = super::tests::server(&asset, bytes, "0".repeat(64), "1.1.0");
    assert!(runtime
        .block_on(download(&client, &old, &work, &corrupt.url))
        .is_err());
    assert_eq!(
        fs::read_to_string(f.old.join(Manifest::executable())).unwrap(),
        "old"
    );
    assert!(!f.old.join(super::super::STATE).exists());
}
#[test]
fn archive_rejects_path_escape_and_symbolic_links() {
    let f = super::super::tests::Fixture::new();
    let file = f.directory.join("unsafe.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&file).unwrap());
    zip.start_file(
        "package\\..\\..\\escape",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(b"bad").unwrap();
    zip.finish().unwrap();
    assert!(archive::extract(&file, &f.directory.join("extract"), "package", true).is_err());
    assert!(!f.directory.join("escape").exists());
    let file = f.directory.join("unsafe.tar.gz");
    let gzip = flate2::write::GzEncoder::new(
        fs::File::create(&file).unwrap(),
        flate2::Compression::default(),
    );
    let mut tar = tar::Builder::new(gzip);
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_size(0);
    header.set_mode(0o777);
    tar.append_link(&mut header, "package/web/linked", "/outside")
        .unwrap();
    tar.into_inner().unwrap().finish().unwrap();
    assert!(archive::extract(&file, &f.directory.join("extract-tar"), "package", false).is_err());
}

#[test]
#[ignore = "requires FLOWBASE_TEST_ARCHIVE_PATH and FLOWBASE_TEST_PACKAGE_ROOT"]
fn packaged_archive_downloads_and_extracts() {
    use sha2::{Digest, Sha256};
    let package = PathBuf::from(std::env::var_os("FLOWBASE_TEST_PACKAGE_ROOT").unwrap());
    let archive = PathBuf::from(std::env::var_os("FLOWBASE_TEST_ARCHIVE_PATH").unwrap());
    let f = super::super::tests::Fixture::new();
    let mut old = Manifest::read(&package).unwrap();
    let version = old.version.clone();
    old.version = "0.0.0-native.0".into();
    old.build_id.push_str("-download-baseline-fixture");
    let bytes = fs::read(&archive).unwrap();
    let digest = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let server = server(
        archive.file_name().unwrap().to_str().unwrap(),
        bytes,
        digest,
        &version,
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .unwrap();
    let extracted = runtime
        .block_on(download(
            &client,
            &old,
            &f.directory.join("actual-archive"),
            &server.url,
        ))
        .unwrap()
        .unwrap();
    let manifest = Manifest::read(&extracted).unwrap();
    assert_eq!(manifest.version, version);
    manifest.verify(&extracted).unwrap();
    assert!(extracted.join(".env").is_file());
    assert!(!extracted.join("data").exists());
}

#[test]
fn windows_zip_separators_are_normalized_before_path_validation() {
    let f = super::super::tests::Fixture::new();
    let file = f.directory.join("windows-separators.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&file).unwrap());
    for name in [
        Manifest::executable(),
        ".env.example",
        "web/index.html",
        super::super::manifest::NAME,
    ] {
        zip.start_file(
            format!("package/{name}").replace('/', "\\"),
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(&fs::read(f.new.join(name)).unwrap()).unwrap();
    }
    zip.finish().unwrap();
    let extracted = archive::extract(
        &file,
        &f.directory.join("windows-extracted"),
        "package",
        true,
    )
    .unwrap();
    Manifest::read(&extracted)
        .unwrap()
        .verify(&extracted)
        .unwrap();
}
