use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use hmac::{Hmac, KeyInit, Mac};
use reqwest::Method;
use sha2::{Digest, Sha256};
use storage_object::{
    drivers::rustfs::{
        encode_uri_component, parse_config, sign_s3_request_at, RustfsFileStorageDriver,
        RUSTFS_MULTIPART_CHUNK_BYTES,
    },
    DeleteObjectInput, FileStorageDriver, FileStorageError, FileStoragePutInput,
    FileStoragePutStreamInput, GenerateAccessUrlInput, OpenReadInput, VerifyReadUnchangedInput,
};
use time::macros::datetime;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug)]
struct StoredObject {
    bytes: Vec<u8>,
    content_type: Option<String>,
    etag: String,
    version_id: String,
}

#[derive(Default)]
struct MockS3State {
    objects: HashMap<String, StoredObject>,
    multipart_parts: HashMap<String, Vec<(i32, String, Vec<u8>)>>,
    multipart_content_types: HashMap<String, Option<String>>,
    aborted_uploads: Vec<String>,
    recorded_requests: Vec<(String, String)>,
    next_version: u64,
    fail_upload_part: Option<i32>,
    complete_error_in_200: bool,
    no_such_key_as_400_xml: bool,
}

struct MockS3Server {
    endpoint: String,
    state: Arc<Mutex<MockS3State>>,
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for MockS3Server {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hmac_bytes(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = <HmacSha256 as KeyInit>::new_from_slice(key).unwrap();
    mac.update(data);
    let out = mac.finalize().into_bytes();
    let mut arr = [0_u8; 32];
    arr.copy_from_slice(&out);
    arr
}

fn verify_sigv4(
    method: &str,
    target: &str,
    headers: &HashMap<String, String>,
    body: &[u8],
    access_key: &str,
    secret_key: &str,
    region: &str,
) -> Result<(), String> {
    let payload_sha = hex_lower(&Sha256::digest(body));
    let got_sha = headers
        .get("x-amz-content-sha256")
        .ok_or("missing x-amz-content-sha256")?;
    if got_sha != &payload_sha {
        return Err(format!("payload hash mismatch: {got_sha} != {payload_sha}"));
    }
    let amz_date = headers.get("x-amz-date").ok_or("missing x-amz-date")?;
    if amz_date.len() != 16 || !amz_date.ends_with('Z') {
        return Err(format!("invalid x-amz-date: {amz_date}"));
    }
    let auth = headers
        .get("authorization")
        .ok_or("missing authorization")?;
    let rest = auth
        .strip_prefix("AWS4-HMAC-SHA256 ")
        .ok_or("wrong auth scheme")?;
    let mut kv = HashMap::new();
    for part in rest.split(", ") {
        let (k, v) = part.split_once('=').ok_or("bad auth kv")?;
        kv.insert(k, v);
    }
    let date_stamp = &amz_date[..8];
    let scope = format!("{date_stamp}/{region}/s3/aws4_request");
    let expected_cred = format!("{access_key}/{scope}");
    if kv.get("Credential").copied() != Some(expected_cred.as_str()) {
        return Err(format!("credential mismatch: {:?}", kv.get("Credential")));
    }
    let signed_headers = *kv.get("SignedHeaders").ok_or("missing SignedHeaders")?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut canonical_headers = String::new();
    for name in signed_headers.split(';') {
        let val = headers
            .get(name)
            .ok_or_else(|| format!("missing signed header {name}"))?;
        canonical_headers.push_str(name);
        canonical_headers.push(':');
        canonical_headers.push_str(val.trim());
        canonical_headers.push('\n');
    }
    let canonical_request =
        format!("{method}\n{path}\n{query}\n{canonical_headers}\n{signed_headers}\n{payload_sha}");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        hex_lower(&Sha256::digest(canonical_request.as_bytes()))
    );
    let k_date = hmac_bytes(
        format!("AWS4{secret_key}").as_bytes(),
        date_stamp.as_bytes(),
    );
    let k_region = hmac_bytes(&k_date, region.as_bytes());
    let k_service = hmac_bytes(&k_region, b"s3");
    let k_signing = hmac_bytes(&k_service, b"aws4_request");
    let expected_sig = hex_lower(&hmac_bytes(&k_signing, string_to_sign.as_bytes()));
    if kv.get("Signature").copied() != Some(expected_sig.as_str()) {
        return Err(format!(
            "signature mismatch: got {:?}, expected {expected_sig}",
            kv.get("Signature")
        ));
    }
    Ok(())
}

async fn read_http_request(
    stream: &mut TcpStream,
) -> std::io::Result<Option<(String, String, HashMap<String, String>, Vec<u8>)>> {
    let mut buf = Vec::new();
    let header_end = loop {
        let mut chunk = [0_u8; 4096];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            if buf.is_empty() {
                return Ok(None);
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "incomplete HTTP headers",
            ));
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
    };
    let header_text = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let content_length = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = header_end + 4;
    let mut body = buf[body_start..].to_vec();
    while body.len() < content_length {
        let mut chunk = vec![0_u8; (content_length - body.len()).min(65536)];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "incomplete HTTP body",
            ));
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);
    Ok(Some((method, target, headers, body)))
}

async fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    extra_headers: &[(&str, String)],
    body: &[u8],
    is_head: bool,
) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {status} {reason}\r\nConnection: close\r\n");
    if !extra_headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("content-length"))
    {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    for (k, v) in extra_headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).await?;
    if !is_head && !body.is_empty() {
        stream.write_all(body).await?;
    }
    stream.flush().await
}

fn percent_decode_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(val) = u8::from_str_radix(hex, 16) {
                    out.push(val);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

async fn start_mock_s3_server() -> MockS3Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let expected_host = format!("127.0.0.1:{}", addr.port());
    let state = Arc::new(Mutex::new(MockS3State::default()));
    let state_clone = state.clone();

    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let state = state_clone.clone();
            let expected_host = expected_host.clone();
            tokio::spawn(async move {
                let Ok(Some((method, target, headers, body))) =
                    read_http_request(&mut stream).await
                else {
                    return;
                };
                let is_head = method == "HEAD";
                if headers.get("host").map(String::as_str) != Some(expected_host.as_str()) {
                    let _ = write_http_response(
                        &mut stream,
                        400,
                        "Bad Request",
                        &[],
                        b"invalid host header",
                        is_head,
                    )
                    .await;
                    return;
                }
                if let Err(err) = verify_sigv4(
                    &method,
                    &target,
                    &headers,
                    &body,
                    "rustfsadmin",
                    "rustfssecret",
                    "us-east-1",
                ) {
                    let xml = format!(
                        "<Error><Code>SignatureDoesNotMatch</Code><Message>{err}</Message></Error>"
                    );
                    let _ = write_http_response(
                        &mut stream,
                        403,
                        "Forbidden",
                        &[("Content-Type", "application/xml".into())],
                        xml.as_bytes(),
                        is_head,
                    )
                    .await;
                    return;
                }

                let (raw_path, raw_query) = target.split_once('?').unwrap_or((&target, ""));
                let decoded_path = percent_decode_path(raw_path);
                let trimmed = decoded_path.trim_start_matches('/');
                let (bucket, key_opt) = match trimmed.split_once('/') {
                    Some((b, k)) => (b.to_string(), Some(k.to_string())),
                    None => (trimmed.to_string(), None),
                };

                let response = {
                    let mut st = state.lock().unwrap();
                    st.recorded_requests.push((method.clone(), target.clone()));

                    if bucket != "attachments" {
                        (
                            404,
                            "Not Found",
                            vec![],
                            b"<Error><Code>NoSuchBucket</Code></Error>".to_vec(),
                        )
                    } else if key_opt.is_none() && method == "HEAD" {
                        (200, "OK", vec![], Vec::new())
                    } else {
                        let key = key_opt.unwrap_or_default();
                        if method == "POST" && raw_query == "uploads=" {
                            st.next_version += 1;
                            let upload_id = format!("upload-{}", st.next_version);
                            st.multipart_parts.insert(upload_id.clone(), Vec::new());
                            st.multipart_content_types
                                .insert(upload_id.clone(), headers.get("content-type").cloned());
                            let xml = format!(
                                "<InitiateMultipartUploadResult><Bucket>{bucket}</Bucket><Key>{key}</Key><UploadId>{upload_id}</UploadId></InitiateMultipartUploadResult>"
                            );
                            (
                                200,
                                "OK",
                                vec![("Content-Type", "application/xml".into())],
                                xml.into_bytes(),
                            )
                        } else if method == "PUT" && raw_query.starts_with("partNumber=") {
                            let mut part_num = 0_i32;
                            let mut upload_id = String::new();
                            for kv in raw_query.split('&') {
                                if let Some((k, v)) = kv.split_once('=') {
                                    if k == "partNumber" {
                                        part_num = v.parse().unwrap_or(0);
                                    } else if k == "uploadId" {
                                        upload_id = v.to_string();
                                    }
                                }
                            }
                            if st.fail_upload_part == Some(part_num) {
                                (
                                    500,
                                    "Internal Server Error",
                                    vec![],
                                    b"<Error><Code>InternalError</Code></Error>".to_vec(),
                                )
                            } else {
                                let etag = format!("\"{}\"", hex_lower(&Sha256::digest(&body)));
                                st.multipart_parts.entry(upload_id).or_default().push((
                                    part_num,
                                    etag.clone(),
                                    body,
                                ));
                                (200, "OK", vec![("ETag", etag)], Vec::new())
                            }
                        } else if method == "POST" && raw_query.starts_with("uploadId=") {
                            let upload_id = raw_query.trim_start_matches("uploadId=").to_string();
                            if st.complete_error_in_200 {
                                (
                                    200,
                                    "OK",
                                    vec![("Content-Type", "application/xml".into())],
                                    b"<Error><Code>InternalError</Code><Message>async failure</Message></Error>".to_vec(),
                                )
                            } else if let Some(mut parts) = st.multipart_parts.remove(&upload_id) {
                                let body_str = String::from_utf8_lossy(&body);
                                assert!(body_str.starts_with("<CompleteMultipartUpload>"));
                                parts.sort_by_key(|(n, _, _)| *n);
                                let mut combined = Vec::new();
                                for (_, _, chunk) in parts {
                                    combined.extend_from_slice(&chunk);
                                }
                                st.next_version += 1;
                                let etag = format!("\"{}\"", hex_lower(&Sha256::digest(&combined)));
                                let version_id = format!("v{}", st.next_version);
                                let ct = st.multipart_content_types.remove(&upload_id).flatten();
                                st.objects.insert(
                                    key,
                                    StoredObject {
                                        bytes: combined,
                                        content_type: ct,
                                        etag: etag.clone(),
                                        version_id,
                                    },
                                );
                                let xml = format!(
                                    "<CompleteMultipartUploadResult><ETag>{etag}</ETag></CompleteMultipartUploadResult>"
                                );
                                (
                                    200,
                                    "OK",
                                    vec![("Content-Type", "application/xml".into())],
                                    xml.into_bytes(),
                                )
                            } else {
                                (
                                    404,
                                    "Not Found",
                                    vec![],
                                    b"<Error><Code>NoSuchUpload</Code></Error>".to_vec(),
                                )
                            }
                        } else if method == "DELETE" && raw_query.starts_with("uploadId=") {
                            let upload_id = raw_query.trim_start_matches("uploadId=").to_string();
                            st.multipart_parts.remove(&upload_id);
                            st.aborted_uploads.push(upload_id);
                            (204, "No Content", vec![], Vec::new())
                        } else if method == "PUT" {
                            st.next_version += 1;
                            let etag = format!("\"{}\"", hex_lower(&Sha256::digest(&body)));
                            let version_id = format!("v{}", st.next_version);
                            st.objects.insert(
                                key,
                                StoredObject {
                                    bytes: body,
                                    content_type: headers.get("content-type").cloned(),
                                    etag: etag.clone(),
                                    version_id,
                                },
                            );
                            (200, "OK", vec![("ETag", etag)], Vec::new())
                        } else if method == "DELETE" {
                            st.objects.remove(&key);
                            (204, "No Content", vec![], Vec::new())
                        } else if method == "GET" || method == "HEAD" {
                            if let Some(obj) = st.objects.get(&key) {
                                let mut hdrs = vec![
                                    ("Content-Length", obj.bytes.len().to_string()),
                                    ("ETag", obj.etag.clone()),
                                    ("x-amz-version-id", obj.version_id.clone()),
                                ];
                                if let Some(ct) = &obj.content_type {
                                    hdrs.push(("Content-Type", ct.clone()));
                                }
                                (200, "OK", hdrs, obj.bytes.clone())
                            } else if st.no_such_key_as_400_xml {
                                (
                                    400,
                                    "Bad Request",
                                    vec![("Content-Type", "application/xml".into())],
                                    b"<Error><Code>NoSuchKey</Code></Error>".to_vec(),
                                )
                            } else {
                                (
                                    404,
                                    "Not Found",
                                    vec![("Content-Type", "application/xml".into())],
                                    b"<Error><Code>NoSuchKey</Code></Error>".to_vec(),
                                )
                            }
                        } else {
                            (405, "Method Not Allowed", vec![], Vec::new())
                        }
                    }
                };

                let header_refs: Vec<(&str, String)> = response.2;
                let _ = write_http_response(
                    &mut stream,
                    response.0,
                    response.1,
                    &header_refs,
                    &response.3,
                    is_head,
                )
                .await;
            });
        }
    });

    MockS3Server {
        endpoint: format!("http://{addr}"),
        state,
        handle,
    }
}

fn mock_config(endpoint: &str) -> serde_json::Value {
    serde_json::json!({
        "endpoint": endpoint,
        "bucket": "attachments",
        "access_key": "rustfsadmin",
        "secret_key": "rustfssecret",
        "region": "us-east-1",
    })
}

#[test]
fn rustfs_driver_requires_endpoint_bucket_access_key_and_secret_key() {
    let driver = RustfsFileStorageDriver;

    assert!(driver
        .validate_config(&serde_json::json!({
            "endpoint": "http://127.0.0.1:39000",
            "bucket": "attachments"
        }))
        .is_err());

    for invalid_endpoint in [
        "",
        "not-a-url",
        "ftp://127.0.0.1:39000",
        "http://user:pass@127.0.0.1:39000",
        "http://127.0.0.1:39000?foo=bar",
    ] {
        let error = driver
            .validate_config(&serde_json::json!({
                "endpoint": invalid_endpoint,
                "bucket": "attachments",
                "access_key": "rustfsadmin",
                "secret_key": "rustfsadmin"
            }))
            .unwrap_err();
        assert!(
            matches!(error, FileStorageError::InvalidConfig("endpoint")),
            "endpoint {invalid_endpoint:?} produced {error:?}"
        );
    }

    assert!(driver
        .validate_config(&serde_json::json!({
            "endpoint": "http://127.0.0.1:39000",
            "bucket": "attachments",
            "access_key": "rustfsadmin",
            "secret_key": "rustfsadmin"
        }))
        .is_ok());
}

#[tokio::test]
async fn rustfs_healthcheck_reports_invalid_endpoint_before_network_io() {
    let driver = RustfsFileStorageDriver;
    let error = driver
        .healthcheck(&serde_json::json!({
            "endpoint": "",
            "bucket": "attachments",
            "access_key": "rustfsadmin",
            "secret_key": "rustfsadmin"
        }))
        .await
        .unwrap_err();

    assert_eq!(error.to_string(), "invalid file storage config: endpoint");
}

#[tokio::test]
async fn rustfs_generate_access_url_uses_public_base_url_when_configured() {
    let driver = RustfsFileStorageDriver;

    assert_eq!(
        driver
            .generate_access_url(GenerateAccessUrlInput {
                config_json: &serde_json::json!({
                    "endpoint": "http://127.0.0.1:39000",
                    "bucket": "attachments",
                    "access_key": "rustfsadmin",
                    "secret_key": "rustfsadmin",
                    "public_base_url": "https://cdn.example.com/files/"
                }),
                object_path: "attachments/2026/04/demo.txt",
            })
            .await
            .unwrap(),
        Some("https://cdn.example.com/files/attachments/2026/04/demo.txt".to_string())
    );
}

#[test]
fn sigv4_signing_matches_canonical_encoding_and_virtual_host_or_path_style_rules() {
    assert_eq!(
        encode_uri_component("folder/report v1+final~1.txt", false),
        "folder/report%20v1%2Bfinal~1.txt"
    );
    assert_eq!(encode_uri_component("a/b+c", true), "a%2Fb%2Bc");

    let virtual_cfg = parse_config(&serde_json::json!({
        "endpoint": "https://s3.example.com:8443/prefix/",
        "bucket": "examplebucket",
        "access_key": "AKIAIOSFODNN7EXAMPLE",
        "secret_key": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        "region": "us-east-1",
        "path_style": false,
    }))
    .unwrap();
    let signed = sign_s3_request_at(
        &virtual_cfg,
        &Method::PUT,
        Some("docs/report v1+final.txt"),
        &[("uploadId", "id/1"), ("partNumber", "2")],
        Some("text/plain"),
        b"hello world",
        datetime!(2013-05-24 00:00:00 UTC),
    )
    .unwrap();
    assert_eq!(signed.host_header, "examplebucket.s3.example.com:8443");
    assert_eq!(
        signed.url.as_str(),
        "https://examplebucket.s3.example.com:8443/prefix/docs/report%20v1%2Bfinal.txt?partNumber=2&uploadId=id%2F1"
    );
    assert_eq!(signed.amz_date, "20130524T000000Z");
    assert_eq!(
        signed.payload_hash,
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
    assert!(signed.authorization.starts_with(
        "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, SignedHeaders=content-type;host;x-amz-content-sha256;x-amz-date, Signature="
    ));
}

#[tokio::test]
async fn rustfs_mock_server_crud_healthcheck_snapshot_and_path_style_alias() {
    let server = start_mock_s3_server().await;
    let driver = RustfsFileStorageDriver;
    let config = serde_json::json!({
        "endpoint": server.endpoint,
        "bucket": "attachments",
        "access_key": "rustfsadmin",
        "secret_key": "rustfssecret",
        "path_style": true,
    });

    let health = driver.healthcheck(&config).await.unwrap();
    assert!(health.reachable);
    assert_eq!(health.detail.as_deref(), Some("attachments"));

    let object_path = "workspace/2026/04/report v1+final.txt";
    let stored = driver
        .put_object(FileStoragePutInput {
            config_json: &config,
            object_path,
            content_type: Some("text/plain; charset=utf-8"),
            bytes: b"sigv4-body-payload".to_vec(),
        })
        .await
        .unwrap();
    assert_eq!(stored.path, object_path);

    let read = driver
        .open_read(OpenReadInput {
            config_json: &config,
            object_path,
        })
        .await
        .unwrap();
    assert_eq!(read.bytes, b"sigv4-body-payload");
    assert_eq!(
        read.content_type.as_deref(),
        Some("text/plain; charset=utf-8")
    );

    let mut stream_read = driver
        .open_read_stream(OpenReadInput {
            config_json: &config,
            object_path,
        })
        .await
        .unwrap();
    let mut buf = Vec::new();
    stream_read.reader.read_to_end(&mut buf).await.unwrap();
    assert_eq!(buf, b"sigv4-body-payload");
    driver
        .verify_read_unchanged(VerifyReadUnchangedInput {
            config_json: &config,
            object_path,
            snapshot: &stream_read.snapshot,
        })
        .await
        .unwrap();

    driver
        .put_object(FileStoragePutInput {
            config_json: &config,
            object_path,
            content_type: Some("text/plain; charset=utf-8"),
            bytes: b"mutated".to_vec(),
        })
        .await
        .unwrap();
    let changed = driver
        .verify_read_unchanged(VerifyReadUnchangedInput {
            config_json: &config,
            object_path,
            snapshot: &stream_read.snapshot,
        })
        .await
        .unwrap_err();
    assert!(matches!(changed, FileStorageError::ObjectChanged));

    driver
        .delete_object(DeleteObjectInput {
            config_json: &config,
            object_path,
        })
        .await
        .unwrap();
    let missing_head = driver
        .verify_read_unchanged(VerifyReadUnchangedInput {
            config_json: &config,
            object_path,
            snapshot: &stream_read.snapshot,
        })
        .await
        .unwrap_err();
    assert!(matches!(missing_head, FileStorageError::ObjectChanged));

    let not_found_404 = driver
        .open_read(OpenReadInput {
            config_json: &config,
            object_path,
        })
        .await
        .unwrap_err();
    assert!(matches!(not_found_404, FileStorageError::ObjectNotFound));

    server.state.lock().unwrap().no_such_key_as_400_xml = true;
    let not_found_xml = driver
        .open_read(OpenReadInput {
            config_json: &config,
            object_path,
        })
        .await
        .unwrap_err();
    assert!(matches!(not_found_xml, FileStorageError::ObjectNotFound));
}

#[tokio::test]
async fn rustfs_put_object_stream_handles_zero_byte_multipart_and_aborts_on_failures() {
    let server = start_mock_s3_server().await;
    let driver = RustfsFileStorageDriver;
    let config = mock_config(&server.endpoint);

    driver
        .put_object_stream(FileStoragePutStreamInput {
            config_json: &config,
            object_path: "streams/empty.bin",
            content_type: Some("application/octet-stream"),
            content_length: 0,
            reader: Box::pin(std::io::Cursor::new(Vec::<u8>::new())),
        })
        .await
        .unwrap();
    let empty = driver
        .open_read(OpenReadInput {
            config_json: &config,
            object_path: "streams/empty.bin",
        })
        .await
        .unwrap();
    assert!(empty.bytes.is_empty());

    let extra_zero = driver
        .put_object_stream(FileStoragePutStreamInput {
            config_json: &config,
            object_path: "streams/non-empty-zero.bin",
            content_type: None,
            content_length: 0,
            reader: Box::pin(std::io::Cursor::new(vec![1_u8])),
        })
        .await
        .unwrap_err();
    assert!(matches!(extra_zero, FileStorageError::ObjectLengthMismatch));

    let mut payload = vec![0x42_u8; RUSTFS_MULTIPART_CHUNK_BYTES + 29];
    payload[0] = 0x11;
    payload[RUSTFS_MULTIPART_CHUNK_BYTES] = 0x99;
    driver
        .put_object_stream(FileStoragePutStreamInput {
            config_json: &config,
            object_path: "streams/multipart.bin",
            content_type: Some("application/octet-stream"),
            content_length: payload.len() as u64,
            reader: Box::pin(std::io::Cursor::new(payload.clone())),
        })
        .await
        .unwrap();
    let restored = driver
        .open_read(OpenReadInput {
            config_json: &config,
            object_path: "streams/multipart.bin",
        })
        .await
        .unwrap();
    assert_eq!(restored.bytes.len(), payload.len());
    assert_eq!(restored.bytes, payload);

    let short_err = driver
        .put_object_stream(FileStoragePutStreamInput {
            config_json: &config,
            object_path: "streams/short.bin",
            content_type: None,
            content_length: 16,
            reader: Box::pin(std::io::Cursor::new(vec![7_u8; 8])),
        })
        .await
        .unwrap_err();
    assert!(matches!(short_err, FileStorageError::ObjectLengthMismatch));
    assert_eq!(server.state.lock().unwrap().aborted_uploads.len(), 1);

    server.state.lock().unwrap().fail_upload_part = Some(1);
    let part_err = driver
        .put_object_stream(FileStoragePutStreamInput {
            config_json: &config,
            object_path: "streams/part-fail.bin",
            content_type: None,
            content_length: 16,
            reader: Box::pin(std::io::Cursor::new(vec![7_u8; 16])),
        })
        .await
        .unwrap_err();
    assert!(matches!(part_err, FileStorageError::Other(_)));
    server.state.lock().unwrap().fail_upload_part = None;
    assert_eq!(server.state.lock().unwrap().aborted_uploads.len(), 2);

    server.state.lock().unwrap().complete_error_in_200 = true;
    let complete_err = driver
        .put_object_stream(FileStoragePutStreamInput {
            config_json: &config,
            object_path: "streams/complete-fail.bin",
            content_type: None,
            content_length: 16,
            reader: Box::pin(std::io::Cursor::new(vec![9_u8; 16])),
        })
        .await
        .unwrap_err();
    assert!(matches!(complete_err, FileStorageError::Other(_)));
    assert_eq!(server.state.lock().unwrap().aborted_uploads.len(), 3);
}
