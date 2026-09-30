//! Request-owned physical blocks. Original part anchors remain durable identities; CAD1 directories share compression.
use super::*;
use anyhow::Context;
use sqlx::postgres::PgRow;
mod codec;
mod directory;
mod manifest;
mod packing;
mod segments;

pub(super) const PART_VERSION: i16 = 2;
pub(super) const PACKED_PART_VERSION: i16 = 3;

struct Anchor {
    id: Uuid,
    offset: i64,
    directory: Vec<u8>,
    checksum: Vec<u8>,
    raw_length: i64,
}

impl PgControlPlaneStore {
    /// Physical maintenance after terminal capture, never a receipt dependency.
    /// Lock the durable head in each bounded transaction, sharing the append
    /// serialization owner. Reentry discovers only still-inline source anchors.
    pub(crate) async fn seal_client_trajectory_archive_request(
        &self,
        request_id: Uuid,
    ) -> Result<u64> {
        let mut sealed = 0;
        let mut committed_first_sequence: Option<i64> = None;
        let mut checked_versions = false;
        loop {
            let mut tx = self.pool().begin().await?;
            let head = sqlx::query("select h.flow_run_id,c.flow_run_id as capture_flow,c.status from client_trajectory_archive_heads h left join client_trajectory_captures c on c.request_id=h.request_id where h.request_id=$1 for update of h")
                .bind(request_id).fetch_optional(&mut *tx).await?;
            let Some(head) = head else {
                tx.commit().await?;
                return Ok(sealed);
            };
            let flow: Option<Uuid> = head.try_get("flow_run_id")?;
            let capture_flow: Option<Uuid> = head.try_get("capture_flow")?;
            let status: Option<String> = head.try_get("status")?;
            if flow.is_none() || status.as_deref() == Some("pending") || status.is_none() {
                tx.commit().await?;
                return Ok(sealed);
            }
            anyhow::ensure!(
                flow == capture_flow,
                "client archive sealing scope mismatch"
            );
            anyhow::ensure!(
                matches!(status.as_deref(), Some("complete" | "incomplete")),
                "client archive capture status unknown"
            );
            if !checked_versions {
                // Normal head-locked append/migration writers only add known
                // codecs. Check retained versions once per invocation rather
                // than rescanning the whole request for each physical block.
                let unknown: bool = sqlx::query_scalar("select exists(select 1 from client_trajectory_archive_parts where request_id=$1 and codec_version not in (0,1,2,3)) or exists(select 1 from client_trajectory_archive_blocks where request_id=$1 and codec_version not in (1,2))")
                    .bind(request_id).fetch_one(&mut *tx).await?;
                anyhow::ensure!(!unknown, "unknown client archive codec version");
                checked_versions = true;
            }
            if let Some(count) = packing::pack_existing(&mut tx, request_id).await? {
                tx.commit().await?;
                sealed += count;
                continue;
            }
            let mut anchors = Vec::new();
            let mut raw = Vec::new();
            let mut directory_bytes = 0usize;
            let mut after = committed_first_sequence;
            loop {
                // Check the declared length before fetching the next body, so an
                // oversized original remains legal without holding two bodies.
                let next: Option<(Uuid, i64, i64, i64)> = sqlx::query_as("select part_id,first_sequence,raw_byte_length,octet_length(frame_directory)::bigint from client_trajectory_archive_parts where request_id=$1 and codec_version=1 and ($2::bigint is null or first_sequence>$2) order by first_sequence limit 1")
                    .bind(request_id).bind(after).fetch_optional(&mut *tx).await?;
                let Some((id, first, length, next_directory_length)) = next else {
                    break;
                };
                let length: usize = length.try_into().context("archive raw length invalid")?;
                let next_directory_length: usize = next_directory_length.try_into()?;
                if !anchors.is_empty()
                    && raw
                        .len()
                        .checked_add(directory_bytes)
                        .and_then(|size| size.checked_add(length))
                        .and_then(|size| size.checked_add(next_directory_length))
                        .context("archive block length overflow")?
                        > codec::TARGET_RAW_BYTES
                {
                    break;
                }
                let row = sqlx::query("select part_id,first_sequence,last_sequence,frames,bytes,codec_version,frame_directory,raw_byte_length,raw_checksum,block_id,block_offset from client_trajectory_archive_parts where part_id=$1 and request_id=$2 for update")
                    .bind(id).bind(request_id).fetch_one(&mut *tx).await?;
                anyhow::ensure!(
                    row.try_get::<i16, _>("codec_version")? == archive_codec::VERSION
                        && row.try_get::<Option<Uuid>, _>("block_id")?.is_none()
                        && row.try_get::<Option<i64>, _>("block_offset")?.is_none(),
                    "archive inline locator invalid"
                );
                let frames: Value = row.try_get("frames")?;
                let bytes: Vec<u8> = row.try_get("bytes")?;
                let directory: Vec<u8> = row.try_get("frame_directory")?;
                let checksum: Vec<u8> = row.try_get("raw_checksum")?;
                let part = archive_codec::Part {
                    version: archive_codec::VERSION,
                    frames: &frames,
                    directory: Some(&directory),
                    bytes: &bytes,
                    raw_byte_length: Some(i64::try_from(length)?),
                    checksum: Some(&checksum),
                    first_sequence: first,
                    last_sequence: row.try_get("last_sequence")?,
                };
                let source = archive_codec::decompress_raw(&part)?;
                archive_codec::decode_raw(&part, &source, i64::MAX, 0)?;
                directory_bytes = directory_bytes
                    .checked_add(directory.len())
                    .context("archive block directory length overflow")?;
                anchors.push(Anchor {
                    id,
                    offset: raw.len().try_into()?,
                    directory,
                    checksum,
                    raw_length: source.len().try_into()?,
                });
                raw.extend_from_slice(&source);
                after = Some(first);
                // Bound both raw size and empty/tiny anchor bookkeeping.
                if raw.len().saturating_add(directory_bytes) >= codec::TARGET_RAW_BYTES
                    || anchors.len() >= 256
                {
                    break;
                }
            }
            if anchors.is_empty() {
                tx.commit().await?;
                return Ok(sealed);
            }
            let mut locators = Vec::with_capacity(anchors.len());
            for anchor in &anchors {
                locators.push(directory::append(&mut raw, &anchor.directory)?);
            }
            let bytes = codec::encode(&raw)?;
            let checksum = codec::checksum(&raw);
            let length: i64 = raw.len().try_into()?;
            let restored = codec::decode(codec::VERSION, &bytes, length, &checksum)?;
            anyhow::ensure!(restored == raw, "archive block original mismatch");
            let block = Uuid::now_v7();
            sqlx::query("insert into client_trajectory_archive_blocks(block_id,request_id,codec_version,bytes,raw_byte_length,raw_checksum) values($1,$2,$3,$4,$5,$6)")
                .bind(block).bind(request_id).bind(codec::PACKED_VERSION).bind(bytes).bind(length).bind(checksum).execute(&mut *tx).await?;
            for (anchor, locator) in anchors.iter().zip(locators) {
                // Immutable source identity and authenticated metadata must still
                // match. Any failed switch rolls back block and every locator.
                let affected = sqlx::query("update client_trajectory_archive_parts set codec_version=3,block_id=$3,block_offset=$4,frame_directory=$8,bytes=''::bytea where part_id=$1 and request_id=$2 and codec_version=1 and block_id is null and block_offset is null and frame_directory=$5 and raw_checksum=$6 and raw_byte_length=$7")
                    .bind(anchor.id).bind(request_id).bind(block).bind(anchor.offset).bind(&anchor.directory).bind(&anchor.checksum).bind(anchor.raw_length).bind(locator)
                    .execute(&mut *tx).await?.rows_affected();
                anyhow::ensure!(affected == 1, "archive sealing source changed");
            }
            tx.commit().await?;
            committed_first_sequence = after;
            sealed += u64::try_from(anchors.len())?;
        }
    }
}

/// One decoded block retained per page; adjacent anchors share its validation.
#[derive(Default)]
pub(super) struct PageCache {
    block: Option<(Uuid, Vec<u8>)>,
}

impl PageCache {
    pub(super) fn cached_block_id(&self) -> Option<Uuid> {
        self.block.as_ref().map(|block| block.0)
    }

    pub(super) fn decode_row(
        &mut self,
        row: &PgRow,
        cursor: i64,
        limit: usize,
    ) -> Result<Vec<DecodedFrame>> {
        let version: i16 = row.try_get("codec_version")?;
        if version == segments::ROW_VERSION {
            return segments::decode_row(self, row, cursor, limit);
        }
        if !matches!(version, PART_VERSION | PACKED_PART_VERSION) {
            if version == archive_codec::VERSION {
                let frames: Value = row.try_get("frames")?;
                let bytes: Vec<u8> = row.try_get("bytes")?;
                let directory: Option<Vec<u8>> = row.try_get("frame_directory")?;
                let checksum: Option<Vec<u8>> = row.try_get("raw_checksum")?;
                let part = archive_codec::Part {
                    version,
                    frames: &frames,
                    bytes: &bytes,
                    directory: directory.as_deref(),
                    checksum: checksum.as_deref(),
                    raw_byte_length: row.try_get("raw_byte_length")?,
                    first_sequence: row.try_get("first_sequence")?,
                    last_sequence: row.try_get("last_sequence")?,
                };
                let raw = archive_codec::decompress_raw(&part)?;
                return archive_codec::decode_raw(&part, &raw, cursor, limit);
            }
            return super::decode_row(row);
        }
        let id: Uuid = row
            .try_get::<Option<Uuid>, _>("block_id")?
            .context("archive block locator missing")?;
        anyhow::ensure!(
            row.try_get::<Vec<u8>, _>("bytes")?.is_empty(),
            "archive block anchor inline body invalid"
        );
        let offset: usize = row
            .try_get::<Option<i64>, _>("block_offset")?
            .context("archive block offset missing")?
            .try_into()
            .context("archive block offset invalid")?;
        // A LEFT JOIN makes absent/foreign bodies explicit corruption. Locator
        // and compressed body always come from the same MVCC statement.
        let block_version: i16 = row
            .try_get::<Option<i16>, _>("block_codec_version")?
            .context("archive backing block missing")?;
        anyhow::ensure!(
            matches!(block_version, codec::VERSION | codec::PACKED_VERSION)
                && (version != PACKED_PART_VERSION || block_version == codec::PACKED_VERSION),
            "unknown client archive block codec version {block_version}"
        );
        if self.block.as_ref().map(|b| b.0) != Some(id) {
            let bytes: Vec<u8> = row.try_get("block_bytes")?;
            let checksum: Vec<u8> = row.try_get("block_checksum")?;
            let raw = codec::decode(
                block_version,
                &bytes,
                row.try_get("block_raw_length")?,
                &checksum,
            )?;
            self.block = Some((id, raw));
        }
        let raw = &self.block.as_ref().context("archive page cache missing")?.1;
        let length: usize = row
            .try_get::<Option<i64>, _>("raw_byte_length")?
            .context("archive raw length missing")?
            .try_into()?;
        let end = offset
            .checked_add(length)
            .context("archive block slice overflow")?;
        let slice = raw
            .get(offset..end)
            .context("archive block slice out of bounds")?;
        let stored_directory: Vec<u8> = row.try_get("frame_directory")?;
        let directory = if version == PACKED_PART_VERSION {
            directory::resolve(&stored_directory, raw)?
        } else {
            stored_directory.as_slice()
        };
        let checksum: Vec<u8> = row.try_get("raw_checksum")?;
        archive_codec::decode_raw(
            &archive_codec::Part {
                version,
                frames: &Value::Null,
                directory: Some(directory),
                bytes: &[],
                raw_byte_length: Some(length.try_into()?),
                checksum: Some(&checksum),
                first_sequence: row.try_get("first_sequence")?,
                last_sequence: row.try_get("last_sequence")?,
            },
            slice,
            cursor,
            limit,
        )
    }
}
