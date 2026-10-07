use super::*;
use base64::Engine;
use control_plane_contracts::ports::{ClientTrajectoryFrameKind, ClientTrajectoryTransport};
#[path = "archive_codec.rs"]
mod archive_codec;
use archive_codec::{DecodedFrame, Format};
#[path = "archive_block/mod.rs"]
mod archive_block;

impl PgControlPlaneStore {
    pub(crate) async fn append_client_trajectory_archive_part(
        &self,
        input: &AppendClientTrajectoryArchiveInput,
    ) -> Result<ClientTrajectoryArchiveReceipt> {
        self.append_archive_values(input, None).await
    }
    async fn append_archive_values(
        &self,
        input: &AppendClientTrajectoryArchiveInput,
        legacy: Option<&Value>,
    ) -> Result<ClientTrajectoryArchiveReceipt> {
        anyhow::ensure!(!input.frames.is_empty(), "client archive empty part");
        let transport = match input.transport {
            ClientTrajectoryTransport::Http => "http",
            ClientTrajectoryTransport::File => "file",
            ClientTrajectoryTransport::Websocket => "websocket",
        };
        let mut tx = self.pool().begin().await?;
        sqlx::query("insert into client_trajectory_archive_heads(request_id,transport,flow_run_id) values($1,$2,(select flow_run_id from client_trajectory_captures where request_id=$1)) on conflict do nothing")
            .bind(input.request_id).bind(transport).execute(&mut *tx).await?;
        let row = sqlx::query("select transport,persisted_through from client_trajectory_archive_heads where request_id=$1 for update")
            .bind(input.request_id).fetch_one(&mut *tx).await?;
        anyhow::ensure!(
            row.get::<String, _>("transport") == transport,
            "client archive transport mismatch"
        );
        // Original part identities move into sealed manifests. Serialize identity
        // admission and inspect both layouts in one statement across publication.
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("client-archive-part:{}", input.part_id))
            .execute(&mut *tx)
            .await?;
        let owners: Vec<Uuid> = sqlx::query_scalar(
            "select request_id from client_trajectory_archive_parts where part_id=$1 union all select request_id from client_trajectory_archive_segments where part_ids @> array[$1]::uuid[]",
        )
        .bind(input.part_id)
        .fetch_all(&mut *tx)
        .await?;
        anyhow::ensure!(owners.len() <= 1, "client archive part identity duplicated");
        let persisted: i64 = row.get("persisted_through");
        if let Some(owner) = owners.first() {
            anyhow::ensure!(
                *owner == input.request_id,
                "client archive part scope mismatch"
            );
            tx.commit().await?;
            return Ok(ClientTrajectoryArchiveReceipt {
                request_id: input.request_id,
                persisted_through: persisted,
            });
        }
        let count = i64::try_from(input.frames.len())?;
        let through = persisted
            .checked_add(count)
            .ok_or_else(|| anyhow::anyhow!("client archive sequence exhausted"))?;
        if let Some(original) = legacy {
            // Historical SQL _client_archive_ref readers require the exact v0
            // layout. Legacy adapter writes keep that same independently useful
            // layout instead of introducing a SQL decompression dependency.
            let mut bytes = Vec::new();
            let mut directory = Vec::new();
            let original = serde_json::to_vec(original)?;
            for (index, frame) in input.frames.iter().enumerate() {
                let sequence = persisted
                    .checked_add(i64::try_from(index)? + 1)
                    .ok_or_else(|| anyhow::anyhow!("client archive sequence exhausted"))?;
                directory.push(serde_json::json!({"sequence":sequence,"kind":frame.kind,"observed_at":frame.observed_at,"offset":bytes.len(),"length":original.len(),"format":"legacy_json"}));
                bytes.extend_from_slice(&original);
            }
            sqlx::query("insert into client_trajectory_archive_parts(part_id,request_id,first_sequence,last_sequence,frames,bytes) values($1,$2,$3,$4,$5,$6)")
                .bind(input.part_id).bind(input.request_id).bind(persisted+1).bind(through).bind(Value::Array(directory)).bind(bytes)
                .execute(&mut *tx).await?;
        } else {
            // A part remains the original durable transaction boundary; neither
            // frame batching nor the commit-before-receipt contract changes.
            let encoded = archive_codec::encode_assigned(&input.frames, persisted)?;
            sqlx::query("insert into client_trajectory_archive_parts(part_id,request_id,first_sequence,last_sequence,frames,bytes,codec_version,frame_directory,raw_byte_length,raw_checksum) values($1,$2,$3,$4,'[]'::jsonb,$5,$6,$7,$8,$9)")
                .bind(input.part_id).bind(input.request_id).bind(persisted+1).bind(through)
                .bind(encoded.bytes).bind(archive_codec::VERSION).bind(encoded.directory)
                .bind(encoded.raw_byte_length).bind(encoded.checksum).execute(&mut *tx).await?;
        }
        sqlx::query(
            "update client_trajectory_archive_heads set persisted_through=$2 where request_id=$1",
        )
        .bind(input.request_id)
        .bind(through)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(ClientTrajectoryArchiveReceipt {
            request_id: input.request_id,
            persisted_through: through,
        })
    }

    pub(crate) async fn discard_unbound_client_trajectory_archive_parts(
        &self,
        request_id: Uuid,
    ) -> Result<()> {
        // Existing part FK cascades from the head. The extra capture guard
        // preserves a trajectory even if its binding transaction is still visible.
        sqlx::query("delete from client_trajectory_archive_heads h where h.request_id=$1 and h.flow_run_id is null and not exists(select 1 from client_trajectory_captures c where c.request_id=h.request_id)")
            .bind(request_id).execute(self.pool()).await?;
        Ok(())
    }

    pub(crate) async fn read_client_trajectory_archive_frames(
        &self,
        request_id: Uuid,
        cursor: i64,
        limit: i64,
    ) -> Result<Vec<ClientTrajectoryArchiveFrame>> {
        self.read_archive_values(request_id, cursor, usize::try_from(limit.clamp(1, 128))?)
            .await?
            .into_iter()
            .map(|stored| {
                let bytes = archive_wire(stored.format, stored.frame.bytes)?;
                Ok(ClientTrajectoryArchiveFrame {
                    bytes,
                    ..stored.frame
                })
            })
            .collect()
    }

    async fn read_archive_values(
        &self,
        request: Uuid,
        cursor: i64,
        limit: usize,
    ) -> Result<Vec<DecodedFrame>> {
        let mut items = Vec::new();
        let mut after_sequence = cursor;
        let mut cache = archive_block::PageCache::default();
        while items.len() < limit {
            // One intersecting segment/legacy part at a time; the whole request
            // is never materialized. Page cursors survive directory publication.
            let row = sqlx::query(include_str!("archive_page.sql"))
                .bind(request)
                .bind(cursor)
                .bind(Some(after_sequence))
                .bind(cache.cached_block_id())
                .fetch_optional(self.pool())
                .await?;
            let Some(row) = row else {
                break;
            };
            let frames = cache.decode_row(&row, after_sequence, limit - items.len())?;
            if frames.is_empty() {
                after_sequence = row.get("last_sequence");
            }
            for frame in frames {
                if frame.frame.sequence > after_sequence {
                    after_sequence = frame.frame.sequence;
                    items.push(frame);
                    if items.len() == limit {
                        break;
                    }
                }
            }
        }
        Ok(items)
    }

    /// Explicit maintenance scope: a bound request in one allowed run. Each
    /// part commits independently, so interruption and reentry are safe. Legacy
    /// JSON parts and any direct SQL archive reference retain their v0 layout.
    pub(crate) async fn migrate_client_trajectory_archive_parts(
        &self,
        flow_run_id: Uuid,
        request_id: Uuid,
    ) -> Result<u64> {
        let valid: bool = sqlx::query_scalar("select exists(select 1 from client_trajectory_archive_heads h join client_trajectory_captures c on c.request_id=h.request_id where h.request_id=$1 and h.flow_run_id=$2 and c.flow_run_id=$2)")
            .bind(request_id).bind(flow_run_id).fetch_one(self.pool()).await?;
        anyhow::ensure!(valid, "client archive migration scope mismatch");
        // Known legacy formats are retained by policy. Exclude them before
        // opening a per-part reference/decode transaction; unknown formats still
        // reach the strict decoder, and Wire parts keep every existing guard.
        let mut after_part: Option<i64> = None;
        let mut migrated = 0;
        loop {
            let mut tx = self.pool().begin().await?;
            let row = sqlx::query("select p.part_id,p.first_sequence,p.last_sequence,p.frames,p.bytes,p.codec_version,p.frame_directory,p.raw_byte_length,p.raw_checksum from client_trajectory_archive_parts p join client_trajectory_archive_heads h on h.request_id=p.request_id join client_trajectory_captures c on c.request_id=p.request_id where p.request_id=$1 and h.flow_run_id=$2 and c.flow_run_id=$2 and p.codec_version=0 and ((not p.frames @> '[{\"format\":\"legacy_json\"}]'::jsonb and not p.frames @> '[{\"format\":\"legacy_payload_json\"}]'::jsonb) or exists(select 1 from jsonb_array_elements(p.frames) frame where jsonb_typeof(frame->'format')='string' and frame->>'format' not in ('wire','legacy_json','legacy_payload_json'))) and ($3::bigint is null or p.first_sequence>$3) order by p.first_sequence limit 1 for update of p")
                .bind(request_id).bind(flow_run_id).bind(after_part).fetch_optional(&mut *tx).await?;
            let Some(row) = row else {
                tx.commit().await?;
                break;
            };
            let part_id: Uuid = row.get("part_id");
            after_part = Some(row.get("first_sequence"));
            let referenced: bool = sqlx::query_scalar("select exists(select 1 from runtime_events where payload->'_client_archive_ref'->>'part_id'=$1)")
                .bind(part_id.to_string()).fetch_one(&mut *tx).await?;
            if referenced {
                tx.commit().await?;
                continue;
            }
            let originals = decode_row(&row)?;
            if originals.iter().any(|frame| frame.format != Format::Wire) {
                tx.commit().await?;
                continue;
            }
            let frames: Vec<_> = originals.into_iter().map(|frame| frame.frame).collect();
            let encoded = archive_codec::encode(&frames)?;
            let empty = Value::Array(Vec::new());
            let restored = archive_codec::decode(archive_codec::Part {
                version: archive_codec::VERSION,
                frames: &empty,
                directory: Some(&encoded.directory),
                bytes: &encoded.bytes,
                raw_byte_length: Some(encoded.raw_byte_length),
                checksum: Some(&encoded.checksum),
                first_sequence: row.get("first_sequence"),
                last_sequence: row.get("last_sequence"),
            })?;
            anyhow::ensure!(
                frames.len() == restored.len()
                    && frames.iter().zip(&restored).all(|(original, restored)| {
                        original.sequence == restored.frame.sequence
                            && original.kind == restored.frame.kind
                            && original.observed_at == restored.frame.observed_at
                            && original.bytes == restored.frame.bytes
                    }),
                "client archive migration original mismatch"
            );
            // Recheck direct SQL references at the switch. The exact source
            // bytes and all frame metadata have been compared above, not just
            // hashes or a JSONB projection.
            let affected = sqlx::query("update client_trajectory_archive_parts p set codec_version=$3,frames='[]'::jsonb,bytes=$4,frame_directory=$5,raw_byte_length=$6,raw_checksum=$7 where p.part_id=$1 and p.request_id=$2 and p.codec_version=0 and not exists(select 1 from runtime_events e where e.payload->'_client_archive_ref'->>'part_id'=p.part_id::text)")
                .bind(part_id).bind(request_id).bind(archive_codec::VERSION).bind(encoded.bytes)
                .bind(encoded.directory).bind(encoded.raw_byte_length).bind(encoded.checksum)
                .execute(&mut *tx).await?.rows_affected();
            tx.commit().await?;
            migrated += affected;
        }
        Ok(migrated)
    }

    pub(super) async fn read_archived_client_raw(
        &self,
        _flow: Uuid,
        request: Uuid,
        step: Uuid,
        cursor: Option<i64>,
        limit: i64,
    ) -> Result<ClientTrajectorySection> {
        // Caller has already checked step, flow and many-to-many node ownership.
        let limit = usize::try_from(limit)?;
        let rows = self
            .read_archive_values(
                request,
                cursor.unwrap_or(0),
                limit
                    .checked_add(1)
                    .ok_or_else(|| anyhow::anyhow!("archive page limit overflow"))?,
            )
            .await?;
        let more = rows.len() > limit;
        let items:Vec<_>=rows.into_iter().take(limit).map(|stored| {
            let frame = stored.frame;
            let bytes = frame.bytes;
            let value=if stored.format != Format::Wire {archive_original_value(stored.format,&bytes)?} else {
                let (encoding,body)=match String::from_utf8(bytes) {Ok(s)=>("utf8",s),Err(error)=>("base64",base64::engine::general_purpose::STANDARD.encode(error.into_bytes()))};
                serde_json::json!({"direction":if frame.kind==ClientTrajectoryFrameKind::Request {"submitted"} else {"emitted"},"encoding":encoding,"body":body,"frame_kind":frame.kind})
            };
            Ok(ClientTrajectorySectionItem {sequence:frame.sequence,value})
        }).collect::<Result<Vec<_>>>()?;
        let next_cursor = if more {
            items.last().map(|i| i.sequence)
        } else {
            None
        };
        Ok(ClientTrajectorySection {
            step_id: step,
            request_id: request,
            evidence_scope: "capture".into(),
            section: "raw".into(),
            items,
            next_cursor,
        })
    }

    pub(super) async fn append_legacy_client_raw(
        &self,
        input: &AppendClientTrajectoryInput,
        value: &Value,
    ) -> Result<()> {
        let valid:bool=sqlx::query_scalar("select exists(select 1 from client_trajectory_captures where request_id=$1 and flow_run_id=$2 and node_run_id is not distinct from $3)")
            .bind(input.request_id).bind(input.flow_run_id).bind(input.node_run_id).fetch_one(self.pool()).await?;
        anyhow::ensure!(valid, "client trajectory capture scope mismatch");
        let bytes = if value["encoding"] == "base64" {
            base64::engine::general_purpose::STANDARD
                .decode(value["body"].as_str().unwrap_or_default())?
        } else {
            value["body"]
                .as_str()
                .unwrap_or_default()
                .as_bytes()
                .to_vec()
        };
        let transport: Option<String> = sqlx::query_scalar(
            "select transport from client_trajectory_archive_heads where request_id=$1",
        )
        .bind(input.request_id)
        .fetch_optional(self.pool())
        .await?;
        let transport = if transport.as_deref() == Some("websocket") {
            ClientTrajectoryTransport::Websocket
        } else {
            ClientTrajectoryTransport::Http
        };
        self.append_archive_values(
            &AppendClientTrajectoryArchiveInput {
                request_id: input.request_id,
                part_id: Uuid::now_v7(),
                transport,
                frames: vec![ClientTrajectoryArchiveFrame {
                    sequence: 0,
                    kind: value
                        .get("frame_kind")
                        .cloned()
                        .map(serde_json::from_value)
                        .transpose()?
                        .unwrap_or(ClientTrajectoryFrameKind::ResponseJson),
                    observed_at: input.observed_at.clone(),
                    bytes,
                }],
            },
            Some(value),
        )
        .await?;
        Ok(())
    }
}

fn decode_row(row: &sqlx::postgres::PgRow) -> Result<Vec<DecodedFrame>> {
    let frames: Value = row.try_get("frames")?;
    let bytes: Vec<u8> = row.try_get("bytes")?;
    let directory: Option<Vec<u8>> = row.try_get("frame_directory")?;
    let checksum: Option<Vec<u8>> = row.try_get("raw_checksum")?;
    archive_codec::decode(archive_codec::Part {
        version: row.try_get("codec_version")?,
        frames: &frames,
        directory: directory.as_deref(),
        bytes: &bytes,
        raw_byte_length: row.try_get("raw_byte_length")?,
        checksum: checksum.as_deref(),
        first_sequence: row.try_get("first_sequence")?,
        last_sequence: row.try_get("last_sequence")?,
    })
}

fn archive_wire(format: Format, bytes: Vec<u8>) -> Result<Vec<u8>> {
    if format == Format::Wire {
        return Ok(bytes);
    }
    let value = archive_original_value(format, &bytes)?;
    if value["encoding"] == "base64" {
        Ok(base64::engine::general_purpose::STANDARD
            .decode(value["body"].as_str().unwrap_or_default())?)
    } else {
        Ok(value["body"]
            .as_str()
            .unwrap_or_default()
            .as_bytes()
            .to_vec())
    }
}

fn archive_original_value(format: Format, bytes: &[u8]) -> Result<Value> {
    let original: Value = serde_json::from_slice(bytes)?;
    if format == Format::LegacyPayloadJson {
        original
            .get("fact")
            .and_then(|f| f.get("value"))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("archived legacy payload value missing"))
    } else {
        Ok(original)
    }
}
