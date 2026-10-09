use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use control_plane_contracts::ports::*;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
mod records;
mod trajectory;
pub(super) use records::records;
pub(super) use trajectory::trajectory;

fn invalid(code: &'static str) -> anyhow::Error {
    ControlPlaneError::InvalidInput(code).into()
}
#[derive(Serialize, Deserialize)]
struct Cursor<T> {
    version: u8,
    query: String,
    position: T,
}
fn decode<T: serde::de::DeserializeOwned>(cursor: Option<&str>, query: &str) -> Result<Option<T>> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    let value: Cursor<T> = URL_SAFE_NO_PAD
        .decode(cursor)
        .ok()
        .and_then(|s| serde_json::from_slice(&s).ok())
        .ok_or_else(|| invalid("log_query_cursor"))?;
    validate_log_query_cursor_binding(value.version, &value.query, query)?;
    Ok(Some(value.position))
}
fn encode<T: Serialize>(position: T, query: &str) -> Result<String> {
    Ok(URL_SAFE_NO_PAD.encode(serde_json::to_vec(&Cursor {
        version: 1,
        query: query.to_owned(),
        position,
    })?))
}
fn timestamp(row: &sqlx::postgres::PgRow, name: &str) -> Result<String> {
    Ok(row
        .try_get::<time::OffsetDateTime, _>(name)?
        .format(&Rfc3339)?)
}
fn optional_timestamp(row: &sqlx::postgres::PgRow, name: &str) -> Result<Option<String>> {
    row.try_get::<Option<time::OffsetDateTime>, _>(name)?
        .map(|t| t.format(&Rfc3339).map_err(Into::into))
        .transpose()
}
