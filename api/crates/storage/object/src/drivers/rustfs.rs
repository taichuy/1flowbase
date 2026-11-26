use std::{
    pin::Pin,
    sync::OnceLock,
    task::{Context, Poll},
};

use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use hmac::{Hmac, KeyInit, Mac};
use reqwest::Method;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};
use url::Url;

use crate::{
    driver::FileStorageDriver,
    errors::{FileStorageError, FileStorageResult},
    types::{
        DeleteObjectInput, FileStorageHealthcheck, FileStorageObjectSnapshot, FileStoragePutInput,
        FileStoragePutResult, FileStoragePutStreamInput, GenerateAccessUrlInput, OpenReadInput,
        OpenReadResult, OpenReadStreamResult, VerifyReadUnchangedInput,
    },
};

#[derive(Debug, Default)]
pub struct RustfsFileStorageDriver;

pub const RUSTFS_MULTIPART_CHUNK_BYTES: usize = 8 * 1024 * 1024;
const RUSTFS_MAX_MULTIPART_PARTS: u64 = 10_000;

static S3_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub(crate) struct RustfsConfig {
    endpoint_url: Url,
    bucket: String,
    access_key: String,
    secret_key: String,
    region: String,
    force_path_style: bool,
    public_base_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SignedS3Request {
    pub(crate) url: Url,
    pub(crate) host_header: String,
    pub(crate) amz_date: String,
    pub(crate) payload_hash: String,
    pub(crate) authorization: String,
    pub(crate) content_type: Option<String>,
}

struct S3StreamReader {
    stream: Pin<Box<dyn Stream<Item = std::io::Result<Vec<u8>>> + Send>>,
    buffer: Vec<u8>,
    offset: usize,
}

impl AsyncRead for S3StreamReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        loop {
            if self.offset < self.buffer.len() {
                let available = &self.buffer[self.offset..];
                let to_copy = available.len().min(buf.remaining());
                buf.put_slice(&available[..to_copy]);
                self.offset += to_copy;
                if self.offset >= self.buffer.len() {
                    self.buffer.clear();
                    self.offset = 0;
                }
                return Poll::Ready(Ok(()));
            }
            match self.stream.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Ready(Some(Err(error))) => return Poll::Ready(Err(error)),
                Poll::Ready(Some(Ok(chunk))) => {
                    if chunk.is_empty() {
                        continue;
                    }
                    self.buffer = chunk;
                    self.offset = 0;
                }
            }
        }
    }
}

fn required_string(
    config_json: &serde_json::Value,
    field: &'static str,
) -> FileStorageResult<String> {
    config_json
        .get(field)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or(FileStorageError::InvalidConfig(field))
}

fn optional_string(config_json: &serde_json::Value, field: &'static str) -> Option<String> {
    config_json
        .get(field)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn optional_bool(config_json: &serde_json::Value, field: &'static str) -> Option<bool> {
    config_json.get(field).and_then(|value| value.as_bool())
}

fn parse_endpoint_url(raw: &str) -> FileStorageResult<Url> {
    let parsed = Url::parse(raw).map_err(|_| FileStorageError::InvalidConfig("endpoint"))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.cannot_be_a_base()
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(FileStorageError::InvalidConfig("endpoint"));
    }
    Ok(parsed)
}

pub(crate) fn parse_config(config_json: &serde_json::Value) -> FileStorageResult<RustfsConfig> {
    let endpoint = required_string(config_json, "endpoint")?;
    let endpoint_url = parse_endpoint_url(&endpoint)?;
    let bucket = required_string(config_json, "bucket")?;
    let access_key = required_string(config_json, "access_key")?;
    let secret_key = required_string(config_json, "secret_key")?;
    let force_path_style = optional_bool(config_json, "force_path_style")
        // @field-contract-compat source=file_storage.config_json.force_path_style alias=path_style remove_by=2027-06-01
        .or_else(|| optional_bool(config_json, "path_style"))
        .unwrap_or(true);
    if !force_path_style && matches!(endpoint_url.host(), Some(url::Host::Ipv6(_))) {
        return Err(FileStorageError::InvalidConfig("endpoint"));
    }
    Ok(RustfsConfig {
        endpoint_url,
        bucket,
        access_key,
        secret_key,
        region: optional_string(config_json, "region").unwrap_or_else(|| "us-east-1".to_string()),
        force_path_style,
        public_base_url: optional_string(config_json, "public_base_url"),
    })
}

fn other_error(error: impl Into<anyhow::Error>) -> FileStorageError {
    FileStorageError::Other(error.into())
}

fn s3_http_client() -> FileStorageResult<&'static reqwest::Client> {
    if let Some(client) = S3_HTTP_CLIENT.get() {
        return Ok(client);
    }
    let _ = rustls::crypto::ring::default_provider().install_default();
    let built = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(other_error)?;
    let _ = S3_HTTP_CLIENT.set(built);
    S3_HTTP_CLIENT
        .get()
        .ok_or_else(|| other_error(anyhow::anyhow!("failed to initialize S3 HTTP client")))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

pub(crate) fn encode_uri_component(input: &str, encode_slash: bool) -> String {
    const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(input.len());
    for &byte in input.as_bytes() {
        let unreserved = byte.is_ascii_uppercase()
            || byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if unreserved || (!encode_slash && byte == b'/') {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(HEX_UPPER[(byte >> 4) as usize] as char);
            out.push(HEX_UPPER[(byte & 0x0f) as usize] as char);
        }
    }
    out
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> FileStorageResult<[u8; 32]> {
    let mut mac = <HmacSha256 as KeyInit>::new_from_slice(key)
        .map_err(|_| other_error(anyhow::anyhow!("invalid S3 signing key")))?;
    mac.update(data);
    let result = mac.finalize().into_bytes();
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&result);
    Ok(bytes)
}

fn format_amz_date(now: OffsetDateTime) -> String {
    let utc = now.to_offset(time::UtcOffset::UTC);
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        utc.year(),
        u8::from(utc.month()),
        utc.day(),
        utc.hour(),
        utc.minute(),
        utc.second()
    )
}

pub(crate) fn sign_s3_request_at(
    config: &RustfsConfig,
    method: &Method,
    object_key: Option<&str>,
    query_params: &[(&str, &str)],
    content_type: Option<&str>,
    payload: &[u8],
    now: OffsetDateTime,
) -> FileStorageResult<SignedS3Request> {
    let endpoint_host = config
        .endpoint_url
        .host_str()
        .ok_or(FileStorageError::InvalidConfig("endpoint"))?;
    let base_path = config.endpoint_url.path().trim_end_matches('/');
    let encoded_bucket = encode_uri_component(&config.bucket, true);
    let normalized_key = match object_key {
        Some(raw) => {
            if raw.trim().is_empty() {
                return Err(FileStorageError::InvalidConfig("object_path"));
            }
            Some(encode_uri_component(
                raw.strip_prefix('/').unwrap_or(raw),
                false,
            ))
        }
        None => None,
    };

    let (target_host, canonical_uri) = if config.force_path_style {
        let uri = match normalized_key {
            Some(encoded_key) => format!("{base_path}/{encoded_bucket}/{encoded_key}"),
            None => format!("{base_path}/{encoded_bucket}"),
        };
        (endpoint_host.to_string(), uri)
    } else {
        let virtual_host = format!("{}.{endpoint_host}", config.bucket);
        let uri = match normalized_key {
            Some(encoded_key) => format!("{base_path}/{encoded_key}"),
            None if base_path.is_empty() => "/".to_string(),
            None => base_path.to_string(),
        };
        (virtual_host, uri)
    };

    let host_header = match config.endpoint_url.port() {
        Some(port) => format!("{target_host}:{port}"),
        None => target_host,
    };

    let mut encoded_query: Vec<(String, String)> = query_params
        .iter()
        .map(|(key, value)| {
            (
                encode_uri_component(key, true),
                encode_uri_component(value, true),
            )
        })
        .collect();
    encoded_query.sort();
    let canonical_query = encoded_query
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");

    let mut raw_url = format!(
        "{}://{host_header}{canonical_uri}",
        config.endpoint_url.scheme()
    );
    if !canonical_query.is_empty() {
        raw_url.push('?');
        raw_url.push_str(&canonical_query);
    }
    let url = Url::parse(&raw_url).map_err(|_| FileStorageError::InvalidConfig("endpoint"))?;

    let amz_date = format_amz_date(now);
    let date_stamp = &amz_date[..8];
    let payload_hash = hex_lower(&Sha256::digest(payload));
    let trimmed_content_type = content_type
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let (canonical_headers, signed_headers) = match trimmed_content_type {
        Some(ct) => (
            format!(
                "content-type:{ct}\nhost:{host_header}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n"
            ),
            "content-type;host;x-amz-content-sha256;x-amz-date",
        ),
        None => (
            format!(
                "host:{host_header}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n"
            ),
            "host;x-amz-content-sha256;x-amz-date",
        ),
    };

    let canonical_request = format!(
        "{}\n{canonical_uri}\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}",
        method.as_str()
    );
    let credential_scope = format!("{date_stamp}/{}/s3/aws4_request", config.region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
        hex_lower(&Sha256::digest(canonical_request.as_bytes()))
    );

    let secret = format!("AWS4{}", config.secret_key);
    let date_key = hmac_sha256(secret.as_bytes(), date_stamp.as_bytes())?;
    let region_key = hmac_sha256(&date_key, config.region.as_bytes())?;
    let service_key = hmac_sha256(&region_key, b"s3")?;
    let signing_key = hmac_sha256(&service_key, b"aws4_request")?;
    let signature = hex_lower(&hmac_sha256(&signing_key, string_to_sign.as_bytes())?);

    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
        config.access_key
    );

    Ok(SignedS3Request {
        url,
        host_header,
        amz_date,
        payload_hash,
        authorization,
        content_type: trimmed_content_type.map(str::to_string),
    })
}

async fn send_s3_request(
    config: &RustfsConfig,
    method: Method,
    object_key: Option<&str>,
    query_params: &[(&str, &str)],
    content_type: Option<&str>,
    body: Vec<u8>,
) -> FileStorageResult<reqwest::Response> {
    let client = s3_http_client()?;
    let signed = sign_s3_request_at(
        config,
        &method,
        object_key,
        query_params,
        content_type,
        &body,
        OffsetDateTime::now_utc(),
    )?;
    let mut builder = client
        .request(method, signed.url)
        .header(reqwest::header::HOST, signed.host_header)
        .header("x-amz-date", signed.amz_date)
        .header("x-amz-content-sha256", signed.payload_hash)
        .header(reqwest::header::AUTHORIZATION, signed.authorization);
    if let Some(ct) = signed.content_type {
        builder = builder.header(reqwest::header::CONTENT_TYPE, ct);
    }
    if !body.is_empty() {
        builder = builder.body(body);
    }
    builder.send().await.map_err(other_error)
}

fn s3_status_error(operation: &str, status: reqwest::StatusCode, body: &str) -> FileStorageError {
    let detail = xml_tag_value(body, "Code")
        .or_else(|| {
            let trimmed = body.trim();
            (!trimmed.is_empty()).then(|| trimmed.chars().take(200).collect())
        })
        .unwrap_or_else(|| "empty error response".to_string());
    other_error(anyhow::anyhow!(
        "S3 {operation} failed with HTTP {}: {detail}",
        status.as_u16()
    ))
}

fn xml_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn xml_unescape(input: &str) -> String {
    input
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn xml_tag_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let rest = &xml[start..];
    let end = rest.find(&close)?;
    let value = xml_unescape(rest[..end].trim());
    (!value.is_empty()).then_some(value)
}

fn response_snapshot(response: &reqwest::Response) -> FileStorageResult<FileStorageObjectSnapshot> {
    let headers = response.headers();
    let content_length = headers
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<i64>().ok())
        .or_else(|| {
            response
                .content_length()
                .and_then(|len| i64::try_from(len).ok())
        });
    let e_tag = headers
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok());
    let version_id = headers
        .get("x-amz-version-id")
        .and_then(|value| value.to_str().ok());
    let content_type = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    rustfs_snapshot(content_length, e_tag, version_id, content_type)
}

fn rustfs_snapshot(
    content_length: Option<i64>,
    e_tag: Option<&str>,
    version_id: Option<&str>,
    content_type: Option<&str>,
) -> FileStorageResult<FileStorageObjectSnapshot> {
    let content_length = content_length
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(FileStorageError::ObjectSnapshotUnavailable)?;
    let e_tag = e_tag.map(str::trim).filter(|value| !value.is_empty());
    let version_id = version_id.map(str::trim).filter(|value| !value.is_empty());
    if e_tag.is_none() && version_id.is_none() {
        return Err(FileStorageError::ObjectSnapshotUnavailable);
    }
    let validator = serde_json::to_string(&serde_json::json!({
        "driver": "rustfs",
        "etag": e_tag,
        "version_id": version_id,
        "content_type": content_type,
    }))
    .map_err(other_error)?;
    Ok(FileStorageObjectSnapshot {
        content_length,
        validator,
    })
}

pub(crate) async fn read_declared_part(
    reader: &mut crate::FileStorageStreamReader,
    length: usize,
) -> FileStorageResult<Vec<u8>> {
    let mut bytes = vec![0_u8; length];
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::UnexpectedEof => FileStorageError::ObjectLengthMismatch,
            _ => other_error(error),
        })?;
    Ok(bytes)
}

async fn ensure_stream_finished(
    reader: &mut crate::FileStorageStreamReader,
) -> FileStorageResult<()> {
    let mut extra = [0_u8; 1];
    if reader.read(&mut extra).await.map_err(other_error)? == 0 {
        Ok(())
    } else {
        Err(FileStorageError::ObjectLengthMismatch)
    }
}

fn rustfs_put_result(config: &RustfsConfig, object_path: &str) -> FileStoragePutResult {
    let url = config
        .public_base_url
        .as_ref()
        .map(|base| format!("{}/{}", base.trim_end_matches('/'), object_path));
    FileStoragePutResult {
        path: object_path.to_string(),
        url,
        metadata_json: serde_json::json!({
            "driver_type": "rustfs",
            "bucket": config.bucket,
        }),
    }
}

#[async_trait]
impl FileStorageDriver for RustfsFileStorageDriver {
    fn driver_type(&self) -> &'static str {
        "rustfs"
    }

    fn validate_config(&self, config_json: &serde_json::Value) -> FileStorageResult<()> {
        let _ = parse_config(config_json)?;
        Ok(())
    }

    async fn healthcheck(
        &self,
        config_json: &serde_json::Value,
    ) -> FileStorageResult<FileStorageHealthcheck> {
        let config = parse_config(config_json)?;
        let response = send_s3_request(&config, Method::HEAD, None, &[], None, Vec::new()).await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(s3_status_error("HeadBucket", status, &body));
        }
        Ok(FileStorageHealthcheck {
            reachable: true,
            detail: Some(config.bucket),
        })
    }

    async fn put_object(
        &self,
        input: FileStoragePutInput<'_>,
    ) -> FileStorageResult<FileStoragePutResult> {
        let config = parse_config(input.config_json)?;
        let response = send_s3_request(
            &config,
            Method::PUT,
            Some(input.object_path),
            &[],
            input.content_type,
            input.bytes,
        )
        .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(s3_status_error("PutObject", status, &body));
        }
        Ok(rustfs_put_result(&config, input.object_path))
    }

    async fn put_object_stream(
        &self,
        input: FileStoragePutStreamInput<'_>,
    ) -> FileStorageResult<FileStoragePutResult> {
        if input.content_length > (RUSTFS_MULTIPART_CHUNK_BYTES as u64) * RUSTFS_MAX_MULTIPART_PARTS
        {
            return Err(FileStorageError::ObjectTooLarge);
        }
        let config = parse_config(input.config_json)?;
        let mut reader = input.reader;
        if input.content_length == 0 {
            ensure_stream_finished(&mut reader).await?;
            let response = send_s3_request(
                &config,
                Method::PUT,
                Some(input.object_path),
                &[],
                input.content_type,
                Vec::new(),
            )
            .await?;
            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(s3_status_error("PutObject", status, &body));
            }
            return Ok(rustfs_put_result(&config, input.object_path));
        }

        let created_response = send_s3_request(
            &config,
            Method::POST,
            Some(input.object_path),
            &[("uploads", "")],
            input.content_type,
            Vec::new(),
        )
        .await?;
        let created_status = created_response.status();
        let created_body = created_response.text().await.map_err(other_error)?;
        if !created_status.is_success() {
            return Err(s3_status_error(
                "CreateMultipartUpload",
                created_status,
                &created_body,
            ));
        }
        let upload_id = xml_tag_value(&created_body, "UploadId")
            .ok_or(FileStorageError::ObjectSnapshotUnavailable)?;

        let upload_result = async {
            let mut remaining = input.content_length;
            let mut part_number = 1_i32;
            let mut completed_parts = Vec::new();
            while remaining > 0 {
                let part_length =
                    usize::try_from(remaining.min(RUSTFS_MULTIPART_CHUNK_BYTES as u64))
                        .map_err(|_| FileStorageError::ObjectTooLarge)?;
                let bytes = read_declared_part(&mut reader, part_length).await?;
                let part_number_str = part_number.to_string();
                let part_response = send_s3_request(
                    &config,
                    Method::PUT,
                    Some(input.object_path),
                    &[("partNumber", &part_number_str), ("uploadId", &upload_id)],
                    None,
                    bytes,
                )
                .await?;
                let part_status = part_response.status();
                if !part_status.is_success() {
                    let body = part_response.text().await.unwrap_or_default();
                    return Err(s3_status_error("UploadPart", part_status, &body));
                }
                let e_tag = part_response
                    .headers()
                    .get(reqwest::header::ETAG)
                    .and_then(|value| value.to_str().ok())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or(FileStorageError::ObjectSnapshotUnavailable)?
                    .to_string();
                completed_parts.push((part_number, e_tag));
                remaining -= part_length as u64;
                part_number = part_number
                    .checked_add(1)
                    .ok_or(FileStorageError::ObjectTooLarge)?;
            }
            ensure_stream_finished(&mut reader).await?;

            let mut complete_xml = String::from("<CompleteMultipartUpload>");
            for (number, e_tag) in &completed_parts {
                complete_xml.push_str("<Part><PartNumber>");
                complete_xml.push_str(&number.to_string());
                complete_xml.push_str("</PartNumber><ETag>");
                complete_xml.push_str(&xml_escape(e_tag));
                complete_xml.push_str("</ETag></Part>");
            }
            complete_xml.push_str("</CompleteMultipartUpload>");

            let complete_response = send_s3_request(
                &config,
                Method::POST,
                Some(input.object_path),
                &[("uploadId", &upload_id)],
                Some("application/xml"),
                complete_xml.into_bytes(),
            )
            .await?;
            let complete_status = complete_response.status();
            let complete_body = complete_response.text().await.map_err(other_error)?;
            if !complete_status.is_success() || complete_body.contains("<Error>") {
                return Err(s3_status_error(
                    "CompleteMultipartUpload",
                    complete_status,
                    &complete_body,
                ));
            }
            Ok::<(), FileStorageError>(())
        }
        .await;

        if let Err(error) = upload_result {
            let _ = send_s3_request(
                &config,
                Method::DELETE,
                Some(input.object_path),
                &[("uploadId", &upload_id)],
                None,
                Vec::new(),
            )
            .await;
            return Err(error);
        }
        Ok(rustfs_put_result(&config, input.object_path))
    }

    async fn delete_object(&self, input: DeleteObjectInput<'_>) -> FileStorageResult<()> {
        let config = parse_config(input.config_json)?;
        let response = send_s3_request(
            &config,
            Method::DELETE,
            Some(input.object_path),
            &[],
            None,
            Vec::new(),
        )
        .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(s3_status_error("DeleteObject", status, &body));
        }
        Ok(())
    }

    async fn open_read(&self, input: OpenReadInput<'_>) -> FileStorageResult<OpenReadResult> {
        let mut opened = self
            .open_read_stream(OpenReadInput {
                config_json: input.config_json,
                object_path: input.object_path,
            })
            .await?;
        let mut bytes = Vec::new();
        opened
            .reader
            .read_to_end(&mut bytes)
            .await
            .map_err(other_error)?;
        self.verify_read_unchanged(VerifyReadUnchangedInput {
            config_json: input.config_json,
            object_path: input.object_path,
            snapshot: &opened.snapshot,
        })
        .await?;
        Ok(OpenReadResult {
            bytes,
            content_type: opened.content_type,
        })
    }

    async fn open_read_stream(
        &self,
        input: OpenReadInput<'_>,
    ) -> FileStorageResult<OpenReadStreamResult> {
        let config = parse_config(input.config_json)?;
        let response = send_s3_request(
            &config,
            Method::GET,
            Some(input.object_path),
            &[],
            None,
            Vec::new(),
        )
        .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            if status == reqwest::StatusCode::NOT_FOUND || body.contains("<Code>NoSuchKey</Code>") {
                return Err(FileStorageError::ObjectNotFound);
            }
            return Err(s3_status_error("GetObject", status, &body));
        }
        let snapshot = response_snapshot(&response)?;
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let stream = response
            .bytes_stream()
            .map(|chunk| chunk.map(Vec::from).map_err(std::io::Error::other));
        Ok(OpenReadStreamResult {
            reader: Box::pin(S3StreamReader {
                stream: Box::pin(stream),
                buffer: Vec::new(),
                offset: 0,
            }),
            content_type,
            snapshot,
        })
    }

    async fn verify_read_unchanged(
        &self,
        input: VerifyReadUnchangedInput<'_>,
    ) -> FileStorageResult<()> {
        let config = parse_config(input.config_json)?;
        let response = send_s3_request(
            &config,
            Method::HEAD,
            Some(input.object_path),
            &[],
            None,
            Vec::new(),
        )
        .await
        .map_err(|_| FileStorageError::ObjectChanged)?;
        if !response.status().is_success() {
            return Err(FileStorageError::ObjectChanged);
        }
        let current = response_snapshot(&response).map_err(|_| FileStorageError::ObjectChanged)?;
        if current == *input.snapshot {
            Ok(())
        } else {
            Err(FileStorageError::ObjectChanged)
        }
    }

    async fn generate_access_url(
        &self,
        input: GenerateAccessUrlInput<'_>,
    ) -> FileStorageResult<Option<String>> {
        let config = parse_config(input.config_json)?;
        Ok(config
            .public_base_url
            .map(|base| format!("{}/{}", base.trim_end_matches('/'), input.object_path)))
    }
}
