//! Atomically publish a sealed directory and retire its recoverable staging rows.
use super::*;

pub(super) const ROW_VERSION: i16 = 4;

impl PgControlPlaneStore {
    pub(crate) async fn seal_and_publish_client_archive(&self, request: Uuid) -> Result<u64> {
        self.seal_client_trajectory_archive_request(request).await?;
        self.publish_client_archive_segments(request).await
    }

    pub(crate) async fn publish_client_archive_segments(&self, request: Uuid) -> Result<u64> {
        let mut retired = 0;
        loop {
            let mut tx = self.pool().begin().await?;
            let allowed: Option<bool> = sqlx::query_scalar("select coalesce(h.flow_run_id=c.flow_run_id and c.status in ('complete','incomplete'),false) from client_trajectory_archive_heads h left join client_trajectory_captures c on c.request_id=h.request_id where h.request_id=$1 for update of h")
                .bind(request).fetch_optional(&mut *tx).await?;
            if allowed != Some(true) {
                return Ok(retired);
            }
            let block: Option<Uuid> = sqlx::query_scalar("select p.block_id from client_trajectory_archive_parts p where p.request_id=$1 and p.codec_version=3 and not exists(select 1 from client_trajectory_archive_segments s where s.block_id=p.block_id) group by p.block_id order by min(p.first_sequence) limit 1")
                .bind(request).fetch_optional(&mut *tx).await?;
            let Some(block) = block else {
                return Ok(retired);
            };
            let backing = sqlx::query("select codec_version,bytes,raw_byte_length,raw_checksum from client_trajectory_archive_blocks where request_id=$1 and block_id=$2")
                .bind(request).bind(block).fetch_one(&mut *tx).await?;
            anyhow::ensure!(
                backing.get::<i16, _>("codec_version") == codec::PACKED_VERSION,
                "segment backing codec invalid"
            );
            let raw = codec::decode(
                codec::PACKED_VERSION,
                &backing.get::<Vec<u8>, _>("bytes"),
                backing.get("raw_byte_length"),
                &backing.get::<Vec<u8>, _>("raw_checksum"),
            )?;
            let rows = sqlx::query("select part_id,first_sequence,last_sequence,block_offset,raw_byte_length,raw_checksum,frame_directory,codec_version,octet_length(bytes) as inline_length from client_trajectory_archive_parts where request_id=$1 and block_id=$2 order by first_sequence for update")
                .bind(request).bind(block).fetch_all(&mut *tx).await?;
            let mut entries = Vec::with_capacity(rows.len());
            for row in rows {
                anyhow::ensure!(
                    row.get::<i16, _>("codec_version") == PACKED_PART_VERSION
                        && row.get::<i32, _>("inline_length") == 0,
                    "segment source codec invalid"
                );
                let entry = manifest::Entry {
                    id: row.get("part_id"),
                    first: row.get("first_sequence"),
                    last: row.get("last_sequence"),
                    offset: row.get("block_offset"),
                    length: row.get("raw_byte_length"),
                    checksum: row.get("raw_checksum"),
                    locator: row.get("frame_directory"),
                };
                manifest::frames(&entry, &raw, i64::MAX, 0)?;
                entries.push(entry);
            }
            let original = manifest::encode(&entries)?;
            let encoded = codec::encode(&original)?;
            let length = i64::try_from(original.len())?;
            let checksum = codec::checksum(&original);
            anyhow::ensure!(
                codec::decode(codec::VERSION, &encoded, length, &checksum)? == original,
                "segment persisted directory mismatch"
            );
            let first = entries.first().context("segment empty")?.first;
            let last = entries.last().context("segment empty")?.last;
            let ids: Vec<_> = entries.iter().map(|entry| entry.id).collect();
            sqlx::query("insert into client_trajectory_archive_segments(block_id,request_id,first_sequence,last_sequence,frame_count,codec_version,part_ids,directory,directory_raw_length,directory_checksum) values($1,$2,$3,$4,$4-$3+1,1,$5,$6,$7,$8)")
                .bind(block).bind(request).bind(first).bind(last).bind(&ids).bind(encoded).bind(length).bind(checksum)
                .execute(&mut *tx).await?;
            // Publication and staging retirement share the transaction and head lock.
            let count = sqlx::query("delete from client_trajectory_archive_parts where request_id=$1 and block_id=$2 and codec_version=3 and part_id=any($3)")
                .bind(request).bind(block).bind(&ids).execute(&mut *tx).await?.rows_affected();
            anyhow::ensure!(
                count == u64::try_from(ids.len())?,
                "segment source changed during publication"
            );
            tx.commit().await?;
            retired += count;
        }
    }

    /// Expand directories before rolling back to a binary without segment reads.
    /// Explicit scope; original blocks and frame bodies are never deleted.
    pub async fn restore_client_trajectory_archive_directory(
        &self,
        flow: Uuid,
        request: Uuid,
    ) -> Result<u64> {
        let mut restored = 0;
        loop {
            let mut tx = self.pool().begin().await?;
            let owner: Option<Uuid> = sqlx::query_scalar("select flow_run_id from client_trajectory_archive_heads where request_id=$1 for update")
                .bind(request).fetch_optional(&mut *tx).await?.flatten();
            anyhow::ensure!(
                owner == Some(flow),
                "archive directory restoration scope mismatch"
            );
            let row = sqlx::query("select s.block_id,s.first_sequence,s.last_sequence,s.frame_count,s.codec_version as segment_codec_version,s.part_ids,s.directory as frame_directory,s.directory_raw_length as raw_byte_length,s.directory_checksum as raw_checksum,b.codec_version as block_codec_version,b.bytes as block_bytes,b.raw_byte_length as block_raw_length,b.raw_checksum as block_checksum from client_trajectory_archive_segments s join client_trajectory_archive_blocks b on b.request_id=s.request_id and b.block_id=s.block_id where s.request_id=$1 order by s.first_sequence limit 1 for update of s")
                .bind(request).fetch_optional(&mut *tx).await?;
            let Some(row) = row else {
                return Ok(restored);
            };
            let entries = entries(&row)?;
            let raw = codec::decode(
                row.get("block_codec_version"),
                &row.get::<Vec<u8>, _>("block_bytes"),
                row.get("block_raw_length"),
                &row.get::<Vec<u8>, _>("block_checksum"),
            )?;
            let block: Uuid = row.get("block_id");
            for entry in &entries {
                manifest::frames(entry, &raw, i64::MAX, 0)?;
                sqlx::query("insert into client_trajectory_archive_parts(part_id,request_id,first_sequence,last_sequence,frames,bytes,codec_version,frame_directory,raw_byte_length,raw_checksum,block_id,block_offset) values($1,$2,$3,$4,'[]'::jsonb,''::bytea,3,$5,$6,$7,$8,$9)")
                    .bind(entry.id).bind(request).bind(entry.first).bind(entry.last).bind(&entry.locator)
                    .bind(entry.length).bind(&entry.checksum).bind(block).bind(entry.offset).execute(&mut *tx).await?;
            }
            sqlx::query("delete from client_trajectory_archive_segments where request_id=$1 and block_id=$2")
                .bind(request).bind(block).execute(&mut *tx).await?;
            tx.commit().await?;
            restored += u64::try_from(entries.len())?;
        }
    }
}

fn entries(row: &PgRow) -> Result<Vec<manifest::Entry>> {
    anyhow::ensure!(
        row.get::<i16, _>("segment_codec_version") == 1,
        "unknown archive segment codec"
    );
    let raw = codec::decode(
        codec::VERSION,
        &row.get::<Vec<u8>, _>("frame_directory"),
        row.get("raw_byte_length"),
        &row.get::<Vec<u8>, _>("raw_checksum"),
    )?;
    let entries = manifest::decode(&raw)?;
    let first: i64 = row.get("first_sequence");
    let last: i64 = row.get("last_sequence");
    anyhow::ensure!(
        entries.first().map(|e| e.first) == Some(first)
            && entries.last().map(|e| e.last) == Some(last)
            && row.get::<i64, _>("frame_count") == last - first + 1,
        "segment range mismatch"
    );
    let ids: Vec<_> = entries.iter().map(|entry| entry.id).collect();
    anyhow::ensure!(
        ids == row.get::<Vec<Uuid>, _>("part_ids"),
        "segment receipt identities mismatch"
    );
    Ok(entries)
}

pub(super) fn decode_row(
    cache: &mut PageCache,
    row: &PgRow,
    cursor: i64,
    limit: usize,
) -> Result<Vec<DecodedFrame>> {
    let entries = entries(row)?;
    let block: Uuid = row.get("block_id");
    if cache.cached_block_id() != Some(block) {
        let version: i16 = row
            .try_get::<Option<i16>, _>("block_codec_version")?
            .context("segment block missing")?;
        anyhow::ensure!(
            version == codec::PACKED_VERSION,
            "segment backing codec invalid"
        );
        let raw = codec::decode(
            version,
            &row.get::<Vec<u8>, _>("block_bytes"),
            row.get("block_raw_length"),
            &row.get::<Vec<u8>, _>("block_checksum"),
        )?;
        cache.block = Some((block, raw));
    }
    let raw = &cache
        .block
        .as_ref()
        .context("segment page cache missing")?
        .1;
    let mut frames = Vec::new();
    for entry in entries.iter().filter(|entry| entry.last > cursor) {
        frames.extend(manifest::frames(entry, raw, cursor, limit - frames.len())?);
        if frames.len() == limit {
            break;
        }
    }
    Ok(frames)
}
