//! Authenticated receipt/range directory. It contains every original part identity.
use super::*;
use std::collections::BTreeSet;

const MAGIC: &[u8; 4] = b"CAS1";
const FIXED: usize = 16 + 8 * 4 + 32 + 4;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Entry {
    pub id: Uuid,
    pub first: i64,
    pub last: i64,
    pub offset: i64,
    pub length: i64,
    pub checksum: Vec<u8>,
    pub locator: Vec<u8>,
}

pub(super) fn encode(entries: &[Entry]) -> Result<Vec<u8>> {
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&u32::try_from(entries.len())?.to_be_bytes());
    for entry in entries {
        anyhow::ensure!(entry.checksum.len() == 32, "segment part checksum invalid");
        bytes.extend_from_slice(entry.id.as_bytes());
        for value in [entry.first, entry.last, entry.offset, entry.length] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        bytes.extend_from_slice(&entry.checksum);
        bytes.extend_from_slice(&u32::try_from(entry.locator.len())?.to_be_bytes());
        bytes.extend_from_slice(&entry.locator);
    }
    anyhow::ensure!(
        decode(&bytes)? == entries,
        "segment manifest reconstruction mismatch"
    );
    Ok(bytes)
}

pub(super) fn decode(mut bytes: &[u8]) -> Result<Vec<Entry>> {
    anyhow::ensure!(
        take::<4>(&mut bytes)? == *MAGIC,
        "segment manifest version invalid"
    );
    let count = u32::from_be_bytes(take(&mut bytes)?) as usize;
    anyhow::ensure!(
        count > 0 && count <= bytes.len() / FIXED,
        "segment manifest count invalid"
    );
    let mut entries = Vec::with_capacity(count);
    let mut identities = BTreeSet::new();
    let mut previous: Option<i64> = None;
    for _ in 0..count {
        let id = Uuid::from_bytes(take(&mut bytes)?);
        let first = i64::from_be_bytes(take(&mut bytes)?);
        let last = i64::from_be_bytes(take(&mut bytes)?);
        let offset = i64::from_be_bytes(take(&mut bytes)?);
        let length = i64::from_be_bytes(take(&mut bytes)?);
        let checksum = take::<32>(&mut bytes)?.to_vec();
        let size = u32::from_be_bytes(take(&mut bytes)?) as usize;
        let locator = bytes
            .get(..size)
            .context("segment locator truncated")?
            .to_vec();
        bytes = &bytes[size..];
        anyhow::ensure!(identities.insert(id), "segment part identity duplicated");
        anyhow::ensure!(
            first > 0 && last >= first && offset >= 0 && length >= 0 && size > 0,
            "segment part shape invalid"
        );
        if let Some(before) = previous {
            anyhow::ensure!(
                before.checked_add(1) == Some(first),
                "segment sequence coverage invalid"
            );
        }
        previous = Some(last);
        entries.push(Entry {
            id,
            first,
            last,
            offset,
            length,
            checksum,
            locator,
        });
    }
    anyhow::ensure!(bytes.is_empty(), "segment manifest bytes trailing");
    Ok(entries)
}

fn take<const N: usize>(bytes: &mut &[u8]) -> Result<[u8; N]> {
    let value = bytes
        .get(..N)
        .context("segment manifest truncated")?
        .try_into()?;
    *bytes = &bytes[N..];
    Ok(value)
}

pub(super) fn frames(
    entry: &Entry,
    raw: &[u8],
    cursor: i64,
    limit: usize,
) -> Result<Vec<DecodedFrame>> {
    let offset = usize::try_from(entry.offset)?;
    let length = usize::try_from(entry.length)?;
    let slice = raw
        .get(
            offset
                ..offset
                    .checked_add(length)
                    .context("segment slice overflow")?,
        )
        .context("segment slice outside block")?;
    let directory = directory::resolve(&entry.locator, raw)?;
    let (frames, count) = archive_codec::decode_raw_with_count(
        &archive_codec::Part {
            version: PACKED_PART_VERSION,
            frames: &Value::Null,
            bytes: &[],
            directory: Some(directory),
            checksum: Some(&entry.checksum),
            raw_byte_length: Some(entry.length),
            first_sequence: entry.first,
            last_sequence: entry.last,
        },
        slice,
        cursor,
        limit,
    )?;
    anyhow::ensure!(
        i64::try_from(count)? == entry.last - entry.first + 1,
        "segment frame coverage invalid"
    );
    Ok(frames)
}

#[cfg(test)]
#[path = "_tests/manifest.rs"]
mod tests;
