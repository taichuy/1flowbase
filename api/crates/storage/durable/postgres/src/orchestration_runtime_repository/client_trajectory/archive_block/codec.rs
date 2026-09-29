use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::io::Read;

pub(super) const VERSION: i16 = 1;
pub(super) const TARGET_RAW_BYTES: usize = 256 * 1024;

pub(super) fn encode(raw: &[u8]) -> Result<Vec<u8>> {
    Ok(zstd::stream::encode_all(raw, 3)?)
}

pub(super) fn checksum(raw: &[u8]) -> Vec<u8> {
    Sha256::digest(raw).to_vec()
}

pub(super) fn decode(version: i16, bytes: &[u8], length: i64, expected: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        version == VERSION,
        "unknown client archive block codec version {version}"
    );
    let length: usize = length
        .try_into()
        .context("archive block raw length invalid")?;
    ensure!(
        expected.len() == 32,
        "archive block checksum length invalid"
    );
    // BufRead input and single_frame preserve the exact compressed boundary:
    // neither concatenated streams nor ignored trailers are accepted.
    let mut decoder = zstd::stream::read::Decoder::with_buffer(bytes)?.single_frame();
    let mut raw = Vec::new();
    loop {
        let mut buffer = [0; 8192];
        let count = decoder
            .read(&mut buffer)
            .context("archive block compression decode failed")?;
        if count == 0 {
            break;
        }
        ensure!(
            count <= length.saturating_sub(raw.len()),
            "archive block raw length mismatch"
        );
        raw.extend_from_slice(&buffer[..count]);
    }
    ensure!(
        decoder.finish().is_empty(),
        "archive block compressed bytes trailing"
    );
    ensure!(raw.len() == length, "archive block raw length mismatch");
    ensure!(
        checksum(&raw) == expected,
        "archive block checksum mismatch"
    );
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_blocks_preserve_binary_oversized_and_empty_originals() {
        for raw in [
            Vec::new(),
            vec![0, 255, 128],
            vec![b'x'; TARGET_RAW_BYTES + 8192],
        ] {
            let bytes = encode(&raw).unwrap();
            assert_eq!(
                decode(
                    VERSION,
                    &bytes,
                    raw.len().try_into().unwrap(),
                    &checksum(&raw)
                )
                .unwrap(),
                raw
            );
        }
    }

    #[test]
    fn block_rejects_unknown_length_checksum_truncation_and_second_stream() {
        let raw = b"exact\0\xff payload";
        let bytes = encode(raw).unwrap();
        let sum = checksum(raw);
        assert!(decode(99, &bytes, raw.len() as i64, &sum).is_err());
        assert!(decode(VERSION, &bytes, -1, &sum).is_err());
        for delta in [-1, 1] {
            assert!(decode(VERSION, &bytes, raw.len() as i64 + delta, &sum).is_err());
        }
        let mut wrong = sum.clone();
        wrong[0] ^= 1;
        assert!(decode(VERSION, &bytes, raw.len() as i64, &wrong).is_err());
        assert!(decode(VERSION, &bytes, raw.len() as i64, &sum[..31]).is_err());
        for removed in [1, 4, 8] {
            assert!(decode(
                VERSION,
                &bytes[..bytes.len() - removed],
                raw.len() as i64,
                &sum
            )
            .is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode(VERSION, &trailing, raw.len() as i64, &sum).is_err());
        let mut concatenated = bytes.clone();
        concatenated.extend_from_slice(&bytes);
        assert!(decode(
            VERSION,
            &concatenated,
            2 * raw.len() as i64,
            &checksum(&[raw.as_slice(), raw.as_slice()].concat())
        )
        .is_err());
    }
}
