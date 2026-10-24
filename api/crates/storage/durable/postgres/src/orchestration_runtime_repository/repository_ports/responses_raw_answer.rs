use super::*;
use futures_util::TryStreamExt;
use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
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
    let (sender, receiver) = mpsc::channel(2);
    let parser = tokio::task::spawn_blocking(move || {
        parse(ChunkReader {
            receiver,
            current: Cursor::new(Vec::new()),
        })
    });
    // This rare branch scans one original output over the wire. PostgreSQL may
    // materialize it internally; Rust never receives or deserializes a whole body.
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

fn parse(reader: impl Read) -> serde_json::Result<Option<String>> {
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
        Answer::deserialize(&mut decoder)?.0
    } else {
        IgnoredAny::deserialize(&mut decoder)?;
        None
    };
    decoder.end()?;
    Ok(answer)
}

struct Answer(Option<String>);
impl<'de> Deserialize<'de> for Answer {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct Fields;
        impl<'de> Visitor<'de> for Fields {
            type Value = Answer;
            fn expecting(&self, out: &mut std::fmt::Formatter) -> std::fmt::Result {
                out.write_str("an output object")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut fields: M) -> Result<Answer, M::Error> {
                let mut priority = 3;
                let mut selected = None;
                while let Some(key) = fields.next_key::<String>()? {
                    let rank = match key.as_str() {
                        "answer" => 0,
                        "text" => 1,
                        "output" => 2,
                        _ => 3,
                    };
                    if rank < 3 && rank <= priority {
                        priority = rank;
                        // Drop any lower priority answer before reading the replacement.
                        drop(selected.take());
                        selected = fields.next_value::<StringOrNone>()?.0;
                    } else {
                        fields.next_value::<IgnoredAny>()?;
                    }
                }
                Ok(Answer(selected))
            }
        }
        decoder.deserialize_map(Fields)
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
