use super::*;
use futures_util::TryStreamExt;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::io::{self, Cursor, Read};
use tokio::sync::mpsc;

// IO buffering, not an answer/context capacity limit. At most two queued chunks
// and the reader's current chunk cross the async/blocking boundary.
const READ_BUFFER_BYTES: i32 = 64 * 1024;

pub(super) async fn read(
    connection: &mut sqlx::PgConnection,
    workspace_id: Uuid,
    application_id: Uuid,
    api_key_id: Uuid,
    actor_user_id: Uuid,
    flow_run_id: Uuid,
) -> Result<Option<String>> {
    // First select field presence and its last duplicate occurrence while ignoring
    // every value. Only the second pass allocates the one scalar actually needed.
    let target = stream(
        connection,
        workspace_id,
        application_id,
        api_key_id,
        actor_user_id,
        flow_run_id,
        |reader| parse(reader, Inspect),
    )
    .await?;
    let Some(target) = target else {
        return Ok(None);
    };
    stream(
        connection,
        workspace_id,
        application_id,
        api_key_id,
        actor_user_id,
        flow_run_id,
        move |reader| parse(reader, Extract(target)),
    )
    .await
}

async fn stream<T: Send + 'static>(
    connection: &mut sqlx::PgConnection,
    workspace_id: Uuid,
    application_id: Uuid,
    api_key_id: Uuid,
    actor_user_id: Uuid,
    flow_run_id: Uuid,
    parse_pass: impl FnOnce(ChunkReader) -> serde_json::Result<T> + Send + 'static,
) -> Result<T> {
    let (sender, receiver) = mpsc::channel(2);
    let parser = tokio::task::spawn_blocking(move || {
        parse_pass(ChunkReader {
            receiver,
            current: Cursor::new(Vec::new()),
        })
    });
    // Each pass sequentially scans one original output over the wire, in the
    // caller's same read-only snapshot. PostgreSQL may materialize it internally;
    // Rust never receives or deserializes a whole body.
    let mut chunks = sqlx::query(r#"
        with original as materialized (
            select convert_to(f.raw_json_payloads ->> 'output_payload', 'UTF8') as body
            from flow_runs f join applications a on a.id = f.application_id
            where f.id = $1 and a.workspace_id = $2 and f.application_id = $3
                and f.api_key_id = $4 and f.created_by = $5 and f.run_mode = 'published_api_run'
                and (f.import_job_id is null or exists (
                    select 1 from run_archive_import_jobs j where j.id = f.import_job_id and j.status = 'succeeded'))
        )
        select substring(body from offsets.chunk_start for $6) as chunk
        from original cross join lateral (
            -- Sort only byte offsets. Materializing/sorting chunk bodies would
            -- recreate an avoidable full-output buffer on the database side.
            select chunk_start from generate_series(1, octet_length(body), $6) chunk_start
            order by chunk_start
        ) offsets
    "#)
    .bind(flow_run_id).bind(workspace_id).bind(application_id).bind(api_key_id)
    .bind(actor_user_id).bind(READ_BUFFER_BYTES).fetch(&mut *connection);
    let mut database_error = None;
    loop {
        match chunks.try_next().await {
            Ok(Some(row)) => {
                if sender.send(row.get::<Vec<u8>, _>("chunk")).await.is_err() {
                    break;
                }
            }
            Ok(None) => break,
            Err(error) => {
                database_error = Some(error);
                break;
            }
        }
    }
    drop(chunks);
    drop(sender);
    // Even on a database or parse error, close the bridge and join its only owner.
    let parsed = parser.await?;
    if let Some(error) = database_error {
        return Err(error.into());
    }
    Ok(parsed?)
}

struct ChunkReader {
    receiver: mpsc::Receiver<Vec<u8>>,
    current: Cursor<Vec<u8>>,
}
impl Read for ChunkReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        loop {
            let count = self.current.read(buffer)?;
            if count != 0 {
                return Ok(count);
            }
            match self.receiver.blocking_recv() {
                Some(chunk) => self.current = Cursor::new(chunk),
                None => return Ok(0),
            }
        }
    }
}

fn parse<T: Default>(
    reader: impl Read,
    seed: impl for<'de> DeserializeSeed<'de, Value = T>,
) -> serde_json::Result<T> {
    let mut reader = io::BufReader::new(reader);
    // Non-object roots have no answer fields. Let serde skip them without ever
    // allocating a scalar body as a String or building a generic Value.
    let is_object = loop {
        use io::BufRead;
        let available = reader.fill_buf().map_err(serde_json::Error::io)?;
        let Some(&first) = available.first() else {
            break false;
        };
        if matches!(first, b' ' | b'\t' | b'\r' | b'\n') {
            reader.consume(1);
        } else {
            break first == b'{';
        }
    };
    let mut decoder = serde_json::Deserializer::from_reader(reader);
    let answer = if is_object {
        seed.deserialize(&mut decoder)?
    } else {
        IgnoredAny::deserialize(&mut decoder)?;
        T::default()
    };
    decoder.end()?;
    Ok(answer)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Target {
    rank: usize,
    occurrence: usize,
}
fn rank(key: &str) -> usize {
    match key {
        "answer" => 0,
        "text" => 1,
        "output" => 2,
        _ => 3,
    }
}

struct Inspect;
impl<'de> DeserializeSeed<'de> for Inspect {
    type Value = Option<Target>;
    fn deserialize<D: Deserializer<'de>>(self, decoder: D) -> Result<Self::Value, D::Error> {
        decoder.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for Inspect {
    type Value = Option<Target>;
    fn expecting(&self, out: &mut std::fmt::Formatter) -> std::fmt::Result {
        out.write_str("an output object")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut fields: M) -> Result<Self::Value, M::Error> {
        let mut occurrences = [0usize; 3];
        let mut target: Option<Target> = None;
        while let Some(key) = fields.next_key::<String>()? {
            let rank = rank(&key);
            if rank < 3 {
                occurrences[rank] = occurrences[rank]
                    .checked_add(1)
                    .ok_or_else(|| serde::de::Error::custom("answer occurrence overflow"))?;
                if target.is_none_or(|target| rank <= target.rank) {
                    target = Some(Target {
                        rank,
                        occurrence: occurrences[rank],
                    });
                }
            }
            fields.next_value::<IgnoredAny>()?;
        }
        Ok(target)
    }
}

struct Extract(Target);
impl<'de> DeserializeSeed<'de> for Extract {
    type Value = Option<String>;
    fn deserialize<D: Deserializer<'de>>(self, decoder: D) -> Result<Self::Value, D::Error> {
        decoder.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for Extract {
    type Value = Option<String>;
    fn expecting(&self, out: &mut std::fmt::Formatter) -> std::fmt::Result {
        out.write_str("an output object")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut fields: M) -> Result<Self::Value, M::Error> {
        let mut occurrence = 0usize;
        let mut selected = None;
        while let Some(key) = fields.next_key::<String>()? {
            if rank(&key) == self.0.rank {
                occurrence = occurrence
                    .checked_add(1)
                    .ok_or_else(|| serde::de::Error::custom("answer occurrence overflow"))?;
                if occurrence == self.0.occurrence {
                    selected = fields.next_value::<StringOrNone>()?.0;
                    continue;
                }
            }
            fields.next_value::<IgnoredAny>()?;
        }
        Ok(selected)
    }
}

struct StringOrNone(Option<String>);
impl<'de> Deserialize<'de> for StringOrNone {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct Scalar;
        impl<'de> Visitor<'de> for Scalar {
            type Value = StringOrNone;
            fn expecting(&self, out: &mut std::fmt::Formatter) -> std::fmt::Result {
                out.write_str("any JSON value")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(StringOrNone(Some(value.to_owned())))
            }
            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(StringOrNone(Some(value)))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(StringOrNone(None))
            }
            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Self::Value, E> {
                Ok(StringOrNone(None))
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Self::Value, E> {
                Ok(StringOrNone(None))
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Self::Value, E> {
                Ok(StringOrNone(None))
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Self::Value, E> {
                Ok(StringOrNone(None))
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut items: S) -> Result<Self::Value, S::Error> {
                while items.next_element::<IgnoredAny>()?.is_some() {}
                Ok(StringOrNone(None))
            }
            fn visit_map<M: MapAccess<'de>>(self, mut fields: M) -> Result<Self::Value, M::Error> {
                while fields.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(StringOrNone(None))
            }
        }
        decoder.deserialize_any(Scalar)
    }
}

#[cfg(test)]
#[path = "_tests/responses_raw_answer.rs"]
mod tests;
