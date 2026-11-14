use super::ExportTraceServiceRequest;
use anyhow::{bail, Result};
use flate2::{read::MultiGzDecoder, write::GzEncoder, Compression};
use prost::Message;
use std::io::{Read, Write};

pub fn decode(
    content_type: &str,
    content_encoding: Option<&str>,
    bytes: &[u8],
) -> Result<ExportTraceServiceRequest> {
    let expanded;
    let bytes = match content_encoding
        .unwrap_or("identity")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "identity" => bytes,
        "gzip" => {
            let mut decoder = MultiGzDecoder::new(bytes);
            let mut body = Vec::new();
            decoder.read_to_end(&mut body)?;
            expanded = body;
            expanded.as_slice()
        }
        _ => bail!("agent_logs.otlp_content_encoding"),
    };
    match content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "application/json" => Ok(serde_json::from_slice(bytes)?),
        "application/x-protobuf" | "application/protobuf" => {
            Ok(ExportTraceServiceRequest::decode(bytes)?)
        }
        _ => bail!("agent_logs.otlp_content_type"),
    }
}

pub fn encode_json(request: &ExportTraceServiceRequest) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec(request)?)
}

pub fn encode_protobuf(request: &ExportTraceServiceRequest) -> Vec<u8> {
    request.encode_to_vec()
}

pub fn encode_gzip(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    Ok(encoder.finish()?)
}
