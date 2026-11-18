use anyhow::{ensure, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use control_plane_contracts::ports::{AgentLogEvent, AgentLogsBatch, AGENT_LOGS_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub(super) const SOURCE_FACT_ATTRIBUTE: &str = "1flowbase.agent_log.source_fact";
pub(super) const SELF_NAMESPACE: &str = "1flowbase_agent_log";
const SOURCE_FACT_ENCODING: &str = "1flowbase.agent-logs.source-fact/v1;base64,";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceFact {
    pub schema_version: String,
    pub source_id: String,
    pub source_client: String,
    pub event: AgentLogEvent,
}

/// Keep the versioned replay authority in one opaque, portable string leaf.
pub(super) fn encode_source_fact(fact: &SourceFact) -> Result<String> {
    Ok(format!(
        "{SOURCE_FACT_ENCODING}{}",
        STANDARD.encode(serde_json::to_vec(fact)?)
    ))
}

/// A malformed self namespace must never fall through to a lossy generic mapping.
pub(super) fn source_fact(metadata: Option<&Value>) -> Result<Option<SourceFact>> {
    let Some(metadata) = metadata else {
        return Ok(None);
    };
    let object = match metadata {
        Value::String(text) => match serde_json::from_str::<Value>(text) {
            Ok(value) => value,
            Err(error) if text.contains(SELF_NAMESPACE) => return Err(error.into()),
            Err(_) => return Ok(None),
        },
        value => value.clone(),
    };
    let Some(value) = object.get(SELF_NAMESPACE) else {
        ensure!(
            !object
                .as_object()
                .is_some_and(|object| object.keys().any(|key| {
                    key.strip_prefix(SELF_NAMESPACE)
                        .is_some_and(|suffix| suffix.starts_with('.'))
                })),
            "agent_logs.source_fact_flattened"
        );
        return Ok(None);
    };
    let fact: SourceFact = match value {
        Value::String(encoded) => {
            let payload = encoded
                .strip_prefix(SOURCE_FACT_ENCODING)
                .ok_or_else(|| anyhow::anyhow!("agent_logs.source_fact_encoding"))?;
            let bytes = STANDARD.decode(payload)?;
            ensure!(
                STANDARD.encode(&bytes) == payload,
                "agent_logs.source_fact_encoding"
            );
            serde_json::from_slice(&bytes)?
        }
        // Legacy local OTLP files retain an intact, validated nested SourceFact.
        _ => serde_json::from_value(value.clone())?,
    };
    AgentLogsBatch {
        schema_version: fact.schema_version.clone(),
        source_id: fact.source_id.clone(),
        source_client: fact.source_client.clone(),
        events: vec![fact.event.clone()],
    }
    .validate()?;
    Ok(Some(fact))
}

pub(super) fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), canonical(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(array) => Value::Array(array.iter().map(canonical).collect()),
        value => value.clone(),
    }
}

pub(super) fn hash(value: &Value) -> Result<Vec<u8>> {
    Ok(Sha256::digest(serde_json::to_vec(&canonical(value))?).to_vec())
}

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn occurred_at(nanos: u64) -> Result<String> {
    ensure!(nanos > 0, "agent_logs.otlp_timestamp_missing");
    Ok(OffsetDateTime::from_unix_timestamp_nanos(i128::from(nanos))?.format(&Rfc3339)?)
}

pub(super) fn nanos(timestamp: &str) -> Result<u64> {
    let nanos = OffsetDateTime::parse(timestamp, &Rfc3339)?.unix_timestamp_nanos();
    ensure!(
        nanos > 0 && nanos <= i128::from(u64::MAX),
        "agent_logs.otlp_timestamp_range"
    );
    Ok(nanos as u64)
}

#[derive(Default)]
pub(super) struct Batches(BTreeMap<(String, String), BTreeMap<String, AgentLogEvent>>);
impl Batches {
    pub fn push(
        &mut self,
        source_id: String,
        source_client: String,
        event: AgentLogEvent,
    ) -> Result<()> {
        let events = self.0.entry((source_id, source_client)).or_default();
        if let Some(existing) = events.get(&event.event_id) {
            ensure!(
                canonical(&serde_json::to_value(existing)?)
                    == canonical(&serde_json::to_value(&event)?),
                "agent_logs.event_conflict"
            );
        } else {
            events.insert(event.event_id.clone(), event);
        }
        Ok(())
    }
    pub fn finish(self) -> Result<Vec<AgentLogsBatch>> {
        self.0
            .into_iter()
            .map(|((source_id, source_client), events)| {
                let mut events: Vec<_> = events.into_values().collect();
                events.sort_by(|left, right| {
                    (left.sequence, &left.event_id).cmp(&(right.sequence, &right.event_id))
                });
                let batch = AgentLogsBatch {
                    schema_version: AGENT_LOGS_SCHEMA_VERSION.into(),
                    source_id,
                    source_client,
                    events,
                };
                batch.validate()?;
                Ok(batch)
            })
            .collect()
    }
}
