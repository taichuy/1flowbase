//! Upgrade one existing physical block without removing any durable part identity.
use super::*;

pub(super) async fn pack_existing(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    request: Uuid,
) -> Result<Option<u64>> {
    let block = sqlx::query("select block_id,bytes,raw_byte_length,raw_checksum from client_trajectory_archive_blocks where request_id=$1 and codec_version=1 order by block_id limit 1 for update")
        .bind(request).fetch_optional(&mut **tx).await?;
    let Some(block) = block else {
        return Ok(None);
    };
    let id: Uuid = block.try_get("block_id")?;
    let old_bytes: Vec<u8> = block.try_get("bytes")?;
    let old_length: i64 = block.try_get("raw_byte_length")?;
    let old_checksum: Vec<u8> = block.try_get("raw_checksum")?;
    let mut raw = codec::decode(codec::VERSION, &old_bytes, old_length, &old_checksum)?;
    let rows = sqlx::query("select part_id,first_sequence,last_sequence,codec_version,frame_directory,raw_byte_length,raw_checksum,block_offset from client_trajectory_archive_parts where request_id=$1 and block_id=$2 order by block_offset for update")
        .bind(request).bind(id).fetch_all(&mut **tx).await?;
    anyhow::ensure!(!rows.is_empty(), "archive packed block anchors missing");
    let mut end = 0usize;
    let mut anchors = Vec::with_capacity(rows.len());
    for row in rows {
        anyhow::ensure!(
            row.try_get::<i16, _>("codec_version")? == PART_VERSION,
            "archive packed source version invalid"
        );
        let offset: i64 = row.try_get("block_offset")?;
        let length: i64 = row.try_get("raw_byte_length")?;
        anyhow::ensure!(
            usize::try_from(offset)? == end,
            "archive packed source offset invalid"
        );
        let next = end
            .checked_add(usize::try_from(length)?)
            .context("archive packed source overflow")?;
        let directory: Vec<u8> = row.try_get("frame_directory")?;
        let checksum: Vec<u8> = row.try_get("raw_checksum")?;
        archive_codec::decode_raw(
            &archive_codec::Part {
                version: PART_VERSION,
                frames: &Value::Null,
                directory: Some(&directory),
                bytes: &[],
                raw_byte_length: Some(length),
                checksum: Some(&checksum),
                first_sequence: row.try_get("first_sequence")?,
                last_sequence: row.try_get("last_sequence")?,
            },
            raw.get(end..next)
                .context("archive packed source bounds invalid")?,
            i64::MAX,
            0,
        )?;
        anchors.push(Anchor {
            id: row.try_get("part_id")?,
            offset,
            directory,
            checksum,
            raw_length: length,
        });
        end = next;
    }
    anyhow::ensure!(
        end == raw.len(),
        "archive packed source body coverage invalid"
    );
    let mut locators = Vec::with_capacity(anchors.len());
    for anchor in &anchors {
        locators.push(directory::append(&mut raw, &anchor.directory)?);
    }
    let bytes = codec::encode(&raw)?;
    let length: i64 = raw.len().try_into()?;
    let checksum = codec::checksum(&raw);
    anyhow::ensure!(
        codec::decode(codec::PACKED_VERSION, &bytes, length, &checksum)? == raw,
        "archive packed block original mismatch"
    );
    let affected = sqlx::query("update client_trajectory_archive_blocks set codec_version=2,bytes=$3,raw_byte_length=$4,raw_checksum=$5 where request_id=$1 and block_id=$2 and codec_version=1 and bytes=$6 and raw_byte_length=$7 and raw_checksum=$8")
        .bind(request).bind(id).bind(bytes).bind(length).bind(checksum).bind(old_bytes).bind(old_length).bind(old_checksum).execute(&mut **tx).await?.rows_affected();
    anyhow::ensure!(affected == 1, "archive packed block source changed");
    for (anchor, locator) in anchors.iter().zip(locators) {
        let affected = sqlx::query("update client_trajectory_archive_parts set codec_version=3,frame_directory=$3 where request_id=$1 and part_id=$2 and codec_version=2 and block_id=$4 and block_offset=$5 and frame_directory=$6 and raw_checksum=$7 and raw_byte_length=$8")
            .bind(request).bind(anchor.id).bind(locator).bind(id).bind(anchor.offset).bind(&anchor.directory).bind(&anchor.checksum).bind(anchor.raw_length).execute(&mut **tx).await?.rows_affected();
        anyhow::ensure!(affected == 1, "archive packed anchor source changed");
    }
    Ok(Some(anchors.len().try_into()?))
}
