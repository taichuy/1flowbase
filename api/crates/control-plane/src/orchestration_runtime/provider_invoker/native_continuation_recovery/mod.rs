//! On-demand recovery from immutable native facts. Normal continuations carry only locators/proof.
use super::*;
use crate::application_public_api::compat::openai::history::completed_history;
use crate::ports::{NativeContinuationRecoverySource as Source, ProviderTransportPayload};
use anyhow::{ensure, Context};
use std::collections::{BTreeMap, BTreeSet};

const CARRIER: &str = "__host_verified_native_continuation";

fn native_input(body: &Value) -> Result<Vec<Value>> {
    ensure!(
        body.get("conversation").is_none_or(Value::is_null),
        "native_recovery_external_conversation"
    );
    let items = match body.get("input") {
        Some(Value::Array(items)) => items.clone(),
        Some(Value::String(text)) => vec![
            json!({"type":"message","role":"user","content":[{"type":"input_text","text":text}]}),
        ],
        _ => anyhow::bail!("native_recovery_input_missing"),
    };
    ensure!(
        !items.iter().any(|item| item["type"] == "item_reference"),
        "native_recovery_unresolved_reference"
    );
    Ok(items)
}

fn validate_rebuild(
    payload: &ProviderTransportPayload,
    input: &ProviderInvocationInput,
) -> Result<()> {
    let carrier = input
        .run_context
        .get(CARRIER)
        .context("native_recovery_carrier_missing")?;
    let source = payload
        .native_recovery_source()
        .context("native_recovery_source_missing")?;
    let proof = payload
        .native_history()
        .context("native_recovery_proof_missing")?;
    ensure!(
        carrier["original_digest"] == payload.digest()
            && carrier["source"] == serde_json::to_value(source)?
            && &carrier["history"] == proof,
        "native_recovery_carrier_mismatch"
    );
    let transport = input
        .native_transport
        .as_ref()
        .context("native_recovery_body_missing")?;
    ensure!(
        transport.protocol == "openai_responses",
        "native_recovery_protocol_mismatch"
    );
    let rebuilt = ProviderTransportPayload::openai_responses(transport.wire_body.clone())?;
    ensure!(
        carrier["rebuilt_digest"] == rebuilt.digest()
            && transport.digest == rebuilt.digest()
            && transport.size_bytes == rebuilt.size_bytes() as u64,
        "native_recovery_digest_mismatch"
    );
    ensure!(
        rebuilt.wire_body().get("previous_response_id").is_none(),
        "native_recovery_cursor_present"
    );
    let mut original_fields = payload
        .wire_body()
        .as_object()
        .context("native_recovery_body_invalid")?
        .clone();
    let mut rebuilt_fields = rebuilt
        .wire_body()
        .as_object()
        .context("native_recovery_body_invalid")?
        .clone();
    for key in ["input", "previous_response_id"] {
        original_fields.remove(key);
        rebuilt_fields.remove(key);
    }
    ensure!(
        original_fields == rebuilt_fields,
        "native_recovery_configuration_changed"
    );
    let expected: domain::orchestration::ResponsesHistoryProof =
        serde_json::from_value(proof.clone())?;
    expected.validate().map_err(anyhow::Error::msg)?;
    let items = rebuilt.wire_body()["input"]
        .as_array()
        .context("native_recovery_input_invalid")?;
    let delta = native_input(payload.wire_body())?;
    ensure!(
        items.len()
            == expected
                .item_count
                .checked_add(delta.len())
                .context("native_recovery_size_overflow")?,
        "native_recovery_suffix_mismatch"
    );
    ensure!(
        items[expected.item_count..] == delta,
        "native_recovery_suffix_mismatch"
    );
    let actual = completed_history(&json!({"input":&items[..expected.item_count]}), None, &[])?;
    ensure!(
        actual.as_ref() == Some(proof),
        "native_recovery_history_mismatch"
    );
    Ok(())
}

pub(super) fn apply_verified_rebuild(
    payload: Option<&ProviderTransportPayload>,
    input: &mut ProviderInvocationInput,
) -> Result<bool> {
    if !input.run_context.contains_key(CARRIER) {
        return Ok(false);
    }
    validate_rebuild(payload.context("native_recovery_payload_missing")?, input)?;
    // All cursor reintroduction carriers are cleared only after exact body/prefix/suffix proof.
    input.previous_response_id = None;
    if let Some(envelope) = input.client_protocol_envelope.as_mut() {
        if envelope.source_protocol == "openai_responses" {
            envelope.body.remove("previous_response_id");
            envelope.body.remove("conversation");
        }
    }
    Ok(true)
}

pub(super) fn is_verified_rebuild(
    payload: Option<&ProviderTransportPayload>,
    input: &ProviderInvocationInput,
) -> bool {
    input.run_context.contains_key(CARRIER)
        && payload.is_some_and(|payload| validate_rebuild(payload, input).is_ok())
        && input.previous_response_id.is_none()
        && input
            .client_protocol_envelope
            .as_ref()
            .is_none_or(|envelope| {
                envelope.source_protocol != "openai_responses"
                    || ["previous_response_id", "conversation"]
                        .iter()
                        .all(|key| envelope.body.get(*key).is_none_or(Value::is_null))
            })
}

#[derive(Clone)]
struct NativeRound {
    node: Uuid,
    invocation: String,
    attempt: u64,
    prepared: Value,
    body: Value,
    output: BTreeMap<u64, Value>,
    response_id: Option<String>,
    received: bool,
    invalid: bool,
}

fn body(payload: &Value) -> Result<Value> {
    serde_json::from_str(
        payload["body"]
            .as_str()
            .context("native_recovery_original_payload_missing")?,
    )
    .map_err(Into::into)
}

fn rounds(records: &[domain::RuntimeEventRecord]) -> Result<Vec<NativeRound>> {
    let mut active = BTreeMap::<Uuid, NativeRound>::new();
    let mut finished = Vec::new();
    for record in records {
        let p = &record.payload;
        let prepared = record.event_type == "provider_semantic_step"
            && p["kind"] == "model_call"
            && p["direction"] == "prepared";
        let received = record.event_type == "provider_semantic_step"
            && p["kind"] == "model_reply"
            && p["direction"] == "received";
        let failed = record.event_type == "provider_semantic_step" && p["kind"] == "error";
        let done = record.event_type == "provider_output_item_done";
        if !(prepared || received || failed || done) {
            continue;
        }
        ensure!(
            record.trust_level == domain::RuntimeTrustLevel::HostFact,
            "native_recovery_untrusted_source"
        );
        let node = record
            .node_run_id
            .or_else(|| {
                p["node_run_id"]
                    .as_str()
                    .and_then(|id| Uuid::parse_str(id).ok())
            })
            .context("native_recovery_node_missing")?;
        ensure!(
            p.get("flow_run_id")
                .filter(|v| !v.is_null())
                .is_none_or(|id| id.as_str() == Some(record.flow_run_id.to_string().as_str())),
            "native_recovery_flow_mismatch"
        );
        ensure!(
            p.get("node_run_id")
                .filter(|v| !v.is_null())
                .is_none_or(|id| id.as_str() == Some(node.to_string().as_str())),
            "native_recovery_node_mismatch"
        );
        if prepared {
            if let Some(previous) = active.remove(&node) {
                finished.push(previous);
            }
            let prepared_body = body(p)?;
            let wire = prepared_body
                .pointer("/native_request/wire_body")
                .filter(|v| v.is_object())
                .cloned();
            // Semantic-mapped invocations are never native history sources.
            let Some(wire) = wire else {
                continue;
            };
            active.insert(
                node,
                NativeRound {
                    node,
                    invocation: p["invocation_id"]
                        .as_str()
                        .filter(|id| !id.is_empty())
                        .context("native_recovery_invocation_missing")?
                        .into(),
                    attempt: p["provider_attempt_index"]
                        .as_u64()
                        .context("native_recovery_attempt_missing")?,
                    prepared: p.clone(),
                    body: wire,
                    output: BTreeMap::new(),
                    response_id: None,
                    received: false,
                    invalid: p["status"] != "recorded",
                },
            );
            continue;
        }
        let Some(round) = active.get_mut(&node) else {
            continue;
        };
        let identity_matches = p
            .get("invocation_id")
            .is_none_or(|id| id.as_str() == Some(round.invocation.as_str()))
            && p.get("provider_attempt_index")
                .is_none_or(|attempt| attempt.as_u64() == Some(round.attempt));
        if !identity_matches {
            round.invalid = true;
            continue;
        }
        if received || failed {
            if p["invocation_id"].as_str() != Some(round.invocation.as_str())
                || p["provider_attempt_index"].as_u64() != Some(round.attempt)
            {
                round.invalid = true;
                continue;
            }
        }
        if failed {
            round.invalid = true;
        }
        if done {
            let index = p["output_index"]
                .as_u64()
                .context("native_recovery_output_index_missing")?;
            let item = p
                .get("item")
                .filter(|v| v.is_object())
                .context("native_recovery_output_item_missing")?;
            if round.output.insert(index, item.clone()).is_some() {
                round.invalid = true;
            }
        }
        if received {
            if round.received {
                round.invalid = true;
            }
            round.received = true;
            let reply = body(p)?;
            round.invalid |= p["status"] != "recorded"
                || !matches!(
                    reply["result"]["finish_reason"].as_str(),
                    Some("stop" | "tool_call" | "mcp_call")
                );
            round.response_id = reply["result"]["response_id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .map(str::to_owned);
        }
    }
    finished.extend(active.into_values());
    Ok(finished)
}

fn source_from_context(round: &NativeRound) -> Result<Option<Source>> {
    let prepared = body(&round.prepared)?;
    if let Some(source) = prepared
        .pointer("/native_context/predecessor_source")
        .filter(|v| !v.is_null())
    {
        return Ok(Some(serde_json::from_value(source.clone())?));
    }
    let flow = round.prepared["context_flow_run_id"].as_str();
    let response = round.prepared["context_response_id"].as_str();
    match (flow, response) {
        (Some(flow), Some(response)) => Ok(Some(Source {
            flow_run_id: Uuid::parse_str(flow)?,
            response_round_id: Uuid::parse_str(
                response
                    .strip_prefix("resp_")
                    .context("native_recovery_response_id_invalid")?,
            )?,
        })),
        _ => Ok(None),
    }
}

fn validate_provider_binding(actual: &Value, expected: &Value) -> Result<()> {
    // Installation/build identity and physical workers may change during recovery.
    // Only the authorized provider/account configuration and native model own history.
    for key in [
        "provider_configuration_digest",
        "provider_instance_id",
        "provider_code",
        "protocol",
        "model",
    ] {
        ensure!(
            actual.get(key).is_some() && actual.get(key) == expected.get(key),
            "native_recovery_provider_binding_mismatch"
        );
    }
    Ok(())
}

impl<R, H> RuntimeProviderInvoker<R, H>
where
    R: ModelProviderRepository
        + OrchestrationRuntimeRepository
        + PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: ProviderRuntimePort + Clone + Send + Sync,
{
    pub(super) async fn materialize_owned_native_continuation(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        route: &orchestration_runtime::execution_engine::ResolvedProviderRoute,
        mut input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationInput> {
        let payload = self
            .provider_transport_payload
            .as_ref()
            .context("native_recovery_payload_missing")?;
        let mut source = payload
            .native_recovery_source()
            .context("native_recovery_source_missing")?;
        let proof = payload
            .native_history()
            .or(self.native_history.as_ref())
            .context("native_recovery_proof_missing")?;
        self.ensure_continuation_route(runtime, &route.runtime_capabilities)?;
        ensure!(
            input
                .native_transport
                .as_ref()
                .is_some_and(|native| native.wire_body == *payload.wire_body()),
            "native_recovery_selected_body_mismatch"
        );
        let application_id = self
            .flow_execution_context
            .as_ref()
            .context("native_recovery_scope_missing")?
            .data_model
            .application_id;
        let pin = route
            .invocation_pin::<RuntimeProviderInvocationPin>()
            .context("native_recovery_route_missing")?;
        let config = pin
            .effective_runtime_config
            .get()
            .context("native_recovery_configuration_missing")?;
        let expected_binding = json!({
            "provider_configuration_digest": format!("sha256:{:x}", base16ct::HexDisplay(&Sha256::digest(serde_json::to_vec(config)?))),
            "provider_instance_id": pin.instance.id.to_string(), "provider_code":runtime.provider_code,
            "protocol":runtime.protocol, "model":runtime.model, "installation_id":pin.installation.id,
            "plugin_version":pin.installation.plugin_version,"artifact_checksum":pin.installation.artifact.local_checksum,
        });
        let deadline = input
            .run_context
            .get("task_deadline_unix_ms")
            .and_then(Value::as_i64)
            .context("native_recovery_deadline_missing")?;
        let mut cursor = payload.wire_body()["previous_response_id"]
            .as_str()
            .context("native_recovery_cursor_missing")?
            .to_owned();
        let mut visited = BTreeSet::new();
        let mut chain = Vec::<NativeRound>::new();
        loop {
            ensure!(
                OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000 < i128::from(deadline),
                "native_recovery_deadline_exceeded"
            );
            ensure!(visited.insert(source), "native_recovery_cycle");
            let run = self
                .repository
                .get_flow_run(application_id, source.flow_run_id)
                .await?
                .context("native_recovery_source_run_missing")?;
            ensure!(
                run.application_id == application_id,
                "native_recovery_application_mismatch"
            );
            let mut records = Vec::new();
            let mut after = 0;
            loop {
                ensure!(
                    OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000
                        < i128::from(deadline),
                    "native_recovery_deadline_exceeded"
                );
                let page = self
                    .repository
                    .list_runtime_event_durable_page(source.flow_run_id, after, None, 256)
                    .await?;
                if page.is_empty() {
                    break;
                }
                for record in page {
                    ensure!(
                        record.flow_run_id == source.flow_run_id && record.sequence > after,
                        "native_recovery_sequence_invalid"
                    );
                    after = record.sequence;
                    records.push(record);
                }
            }
            let candidates = rounds(&records)?
                .into_iter()
                .filter(|round| round.response_id.as_deref() == Some(cursor.as_str()))
                .collect::<Vec<_>>();
            ensure!(
                candidates.len() == 1,
                "native_recovery_source_ambiguous_or_missing"
            );
            let round = candidates.into_iter().next().expect("one candidate");
            ensure!(
                round.received && !round.invalid,
                "native_recovery_source_incomplete"
            );
            let prepared = body(&round.prepared)?;
            ensure!(
                prepared["provider_code"] == runtime.provider_code
                    && prepared["protocol"] == runtime.protocol
                    && prepared["model"] == runtime.model,
                "native_recovery_provider_mismatch"
            );
            if let Some(context) = prepared.get("native_context").filter(|v| v.is_object()) {
                ensure!(
                    context["workspace_id"] == self.workspace_id.to_string()
                        && context["response_round_id"] == source.response_round_id.to_string(),
                    "native_recovery_scope_mismatch"
                );
                validate_provider_binding(&context["provider_binding"], &expected_binding)?;
            } else {
                // Older prepared facts already have lineage. The durable turn span owns
                // their host response identity and provider-configuration binding.
                let spans = self
                    .repository
                    .list_runtime_spans(source.flow_run_id)
                    .await?;
                let response_id = format!("resp_{}", source.response_round_id);
                let evidence = spans
                    .iter()
                    .filter(|span| {
                        span.node_run_id == Some(round.node)
                            && span
                                .metadata
                                .pointer("/native_response/response_id")
                                .and_then(Value::as_str)
                                == Some(response_id.as_str())
                    })
                    .collect::<Vec<_>>();
                ensure!(
                    evidence.len() == 1,
                    "native_recovery_legacy_binding_missing_or_ambiguous"
                );
                validate_provider_binding(
                    &evidence[0].metadata["native_response"]["binding"],
                    &expected_binding,
                )?;
            }
            let previous_cursor = round
                .body
                .get("previous_response_id")
                .filter(|v| !v.is_null())
                .map(|id| {
                    id.as_str()
                        .map(str::to_owned)
                        .context("native_recovery_cursor_invalid")
                })
                .transpose()?;
            let predecessor = if previous_cursor.is_some() {
                source_from_context(&round)?
                    .context("native_recovery_lineage_missing")
                    .map(Some)?
            } else {
                None
            };
            chain.push(round);
            match (predecessor, previous_cursor) {
                (Some(previous), Some(previous_cursor)) => {
                    source = previous;
                    cursor = previous_cursor;
                }
                _ => break,
            }
        }
        let mut prefix = Vec::new();
        for round in chain.iter().rev() {
            prefix.extend(native_input(&round.body)?);
            prefix.extend(round.output.values().cloned());
        }
        ensure!(
            completed_history(&json!({"input":prefix}), None, &[])?.as_ref() == Some(proof),
            "native_recovery_history_mismatch"
        );
        prefix.extend(native_input(payload.wire_body())?);
        let mut rebuilt_body = payload.wire_body().clone();
        let object = rebuilt_body
            .as_object_mut()
            .context("native_recovery_body_invalid")?;
        object.remove("previous_response_id");
        object.insert("input".into(), Value::Array(prefix));
        let rebuilt = ProviderTransportPayload::openai_responses(rebuilt_body)?;
        input.run_context.insert(CARRIER.into(), json!({"original_digest":payload.digest(),
            "source":payload.native_recovery_source(),"history":proof,"rebuilt_digest":rebuilt.digest()}));
        input.native_transport = Some(
            plugin_framework::provider_contract::ProviderNativeTransport {
                protocol: "openai_responses".into(),
                wire_body: rebuilt.wire_body().clone(),
                digest: rebuilt.digest().into(),
                size_bytes: rebuilt.size_bytes() as u64,
            },
        );
        apply_verified_rebuild(Some(payload), &mut input)?;
        Ok(input)
    }
}

#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
