//! One immutable part is the compression boundary. The binary directory keeps
//! transport frame boundaries and timestamp strings without a JSON projection.
use anyhow::{anyhow, ensure, Context, Result};
use control_plane_contracts::ports::{ClientTrajectoryArchiveFrame, ClientTrajectoryFrameKind};
use flate2::{write::ZlibEncoder, Compression, Decompress, FlushDecompress, Status};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write;

pub(super) const VERSION: i16 = 1;
const MAGIC: &[u8; 4] = b"CAD1";

pub(super) struct EncodedPart {
    pub directory: Vec<u8>,
    pub bytes: Vec<u8>,
    pub raw_byte_length: i64,
    pub checksum: Vec<u8>,
}

pub(super) struct Part<'a> {
    pub version: i16,
    pub frames: &'a Value,
    pub directory: Option<&'a [u8]>,
    pub bytes: &'a [u8],
    pub raw_byte_length: Option<i64>,
    pub checksum: Option<&'a [u8]>,
    pub first_sequence: i64,
    pub last_sequence: i64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Format {
    Wire,
    LegacyJson,
    LegacyPayloadJson,
}

pub(super) struct DecodedFrame {
    pub frame: ClientTrajectoryArchiveFrame,
    pub format: Format,
}

pub(super) fn encode(frames: &[ClientTrajectoryArchiveFrame]) -> Result<EncodedPart> {
    ensure!(!frames.is_empty(), "client archive empty part");
    let mut directory = MAGIC.to_vec();
    put_uint(&mut directory, frames.len().try_into()?);
    let mut raw = Vec::new();
    let mut previous = 0;
    for frame in frames {
        ensure!(frame.sequence > previous, "archive sequence order invalid");
        put_uint(&mut directory, frame.sequence.try_into()?);
        directory.push(match frame.kind {
            ClientTrajectoryFrameKind::Request => 0,
            ClientTrajectoryFrameKind::ResponseJson => 1,
            ClientTrajectoryFrameKind::ResponseSse => 2,
        });
        put_uint(&mut directory, raw.len().try_into()?);
        put_uint(&mut directory, frame.bytes.len().try_into()?);
        put_uint(&mut directory, frame.observed_at.len().try_into()?);
        directory.extend_from_slice(frame.observed_at.as_bytes());
        raw.extend_from_slice(&frame.bytes);
        previous = frame.sequence;
    }
    let raw_byte_length = raw.len().try_into()?;
    let checksum = checksum(&directory, &raw);
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw)?;
    Ok(EncodedPart {
        directory,
        bytes: encoder.finish()?,
        raw_byte_length,
        checksum,
    })
}

pub(super) fn decode(part: Part<'_>) -> Result<Vec<DecodedFrame>> {
    let frames = match part.version {
        0 => decode_original(part.frames, part.bytes)?,
        VERSION => decode_compressed(&part)?,
        other => anyhow::bail!("unknown client archive codec version {other}"),
    };
    ensure!(!frames.is_empty(), "archive directory empty");
    ensure!(
        frames.first().map(|f| f.frame.sequence) == Some(part.first_sequence)
            && frames.last().map(|f| f.frame.sequence) == Some(part.last_sequence),
        "archive part sequence bounds mismatch"
    );
    let mut previous = 0;
    for frame in &frames {
        ensure!(
            frame.frame.sequence > previous,
            "archive sequence order invalid"
        );
        previous = frame.frame.sequence;
    }
    Ok(frames)
}

fn decode_compressed(part: &Part<'_>) -> Result<Vec<DecodedFrame>> {
    let directory = part.directory.context("archive directory missing")?;
    let raw_length: usize = part
        .raw_byte_length
        .context("archive raw length missing")?
        .try_into()
        .context("archive raw length invalid")?;
    let expected_checksum = part.checksum.context("archive checksum missing")?;
    ensure!(
        expected_checksum.len() == 32,
        "archive checksum length invalid"
    );
    // Bound decompression by the recorded original length, never by a task-wide
    // capacity policy. Require StreamEnd so a truncated zlib trailer cannot
    // masquerade as a successful read of the correct number of original bytes.
    let mut decoder = Decompress::new(true);
    let mut raw = Vec::new();
    loop {
        let mut buffer = [0; 8192];
        let before_in = decoder.total_in();
        let before_out = decoder.total_out();
        let input_offset = usize::try_from(before_in)?;
        let status = decoder
            .decompress(
                &part.bytes[input_offset..],
                &mut buffer,
                FlushDecompress::None,
            )
            .context("archive compression decode failed")?;
        let written = usize::try_from(decoder.total_out() - before_out)?;
        raw.extend_from_slice(&buffer[..written]);
        ensure!(raw.len() <= raw_length, "archive raw length mismatch");
        if status == Status::StreamEnd {
            break;
        }
        ensure!(
            decoder.total_in() > before_in || written > 0,
            "archive compressed bytes truncated"
        );
    }
    ensure!(raw.len() == raw_length, "archive raw length mismatch");
    ensure!(
        decoder.total_in() == u64::try_from(part.bytes.len())?,
        "archive compressed bytes truncated or trailing"
    );
    ensure!(
        checksum(directory, &raw) == expected_checksum,
        "archive checksum mismatch"
    );

    let mut reader = DirectoryReader {
        bytes: directory,
        offset: 0,
    };
    ensure!(
        reader.take(MAGIC.len())? == MAGIC,
        "archive directory header invalid"
    );
    let count = reader.size()?;
    // Every entry requires at least sequence, kind, offset, length and time
    // length. Reject impossible counts before reserving or iterating them.
    ensure!(
        count > 0 && count <= reader.remaining() / 5,
        "archive directory count invalid"
    );
    let mut frames = Vec::new();
    let mut previous_end = 0;
    for _ in 0..count {
        let sequence = i64::try_from(reader.uint()?).context("archive sequence invalid")?;
        let kind = match reader.take(1)?[0] {
            0 => ClientTrajectoryFrameKind::Request,
            1 => ClientTrajectoryFrameKind::ResponseJson,
            2 => ClientTrajectoryFrameKind::ResponseSse,
            _ => anyhow::bail!("archive frame kind invalid"),
        };
        let offset = reader.size()?;
        let length = reader.size()?;
        let time_length = reader.size()?;
        let observed_at = std::str::from_utf8(reader.take(time_length)?)
            .context("archive timestamp encoding invalid")?
            .to_owned();
        ensure!(offset == previous_end, "archive frame offset invalid");
        let end = offset
            .checked_add(length)
            .context("archive frame bounds overflow")?;
        let bytes = raw
            .get(offset..end)
            .context("archive frame out of bounds")?
            .to_vec();
        frames.push(DecodedFrame {
            frame: ClientTrajectoryArchiveFrame {
                sequence,
                kind,
                observed_at,
                bytes,
            },
            format: Format::Wire,
        });
        previous_end = end;
    }
    ensure!(reader.remaining() == 0, "archive directory trailing bytes");
    ensure!(
        previous_end == raw.len(),
        "archive directory raw length mismatch"
    );
    Ok(frames)
}

fn decode_original(directory: &Value, raw: &[u8]) -> Result<Vec<DecodedFrame>> {
    let entries = directory
        .as_array()
        .context("archive original directory invalid")?;
    let mut frames = Vec::new();
    let mut previous_end = 0;
    for entry in entries {
        let offset: usize = entry["offset"]
            .as_u64()
            .context("archive frame offset missing")?
            .try_into()?;
        let length: usize = entry["length"]
            .as_u64()
            .context("archive frame length missing")?
            .try_into()?;
        ensure!(offset == previous_end, "archive frame offset invalid");
        let end = offset
            .checked_add(length)
            .context("archive frame bounds overflow")?;
        let bytes = raw
            .get(offset..end)
            .context("archive frame out of bounds")?
            .to_vec();
        let format = match entry["format"].as_str() {
            None | Some("wire") => Format::Wire,
            Some("legacy_json") => Format::LegacyJson,
            Some("legacy_payload_json") => Format::LegacyPayloadJson,
            Some(_) => anyhow::bail!("archive original frame format unknown"),
        };
        frames.push(DecodedFrame {
            frame: ClientTrajectoryArchiveFrame {
                sequence: entry["sequence"]
                    .as_i64()
                    .context("archive sequence missing")?,
                kind: serde_json::from_value(entry["kind"].clone())?,
                observed_at: entry["observed_at"].as_str().unwrap_or_default().to_owned(),
                bytes,
            },
            format,
        });
        previous_end = end;
    }
    ensure!(
        previous_end == raw.len(),
        "archive directory raw length mismatch"
    );
    Ok(frames)
}

fn checksum(directory: &[u8], raw: &[u8]) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(VERSION.to_le_bytes());
    digest.update(directory);
    digest.update(raw);
    digest.finalize().to_vec()
}

fn put_uint(bytes: &mut Vec<u8>, mut value: u64) {
    while value >= 128 {
        bytes.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    bytes.push(value as u8);
}

struct DirectoryReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> DirectoryReader<'a> {
    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(count)
            .context("archive directory bounds overflow")?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .context("archive directory truncated")?;
        self.offset = end;
        Ok(bytes)
    }
    fn uint(&mut self) -> Result<u64> {
        let mut value = 0;
        for index in 0..10 {
            let byte = self.take(1)?[0];
            ensure!(index < 9 || byte <= 1, "archive directory integer overflow");
            value |= u64::from(byte & 127) << (index * 7);
            if byte & 128 == 0 {
                ensure!(
                    index == 0 || byte != 0,
                    "archive directory integer noncanonical"
                );
                return Ok(value);
            }
        }
        Err(anyhow!("archive directory integer overflow"))
    }
    fn size(&mut self) -> Result<usize> {
        self.uint()?
            .try_into()
            .context("archive directory length overflow")
    }
}

#[cfg(test)]
#[path = "_tests/archive_codec.rs"]
mod tests;
