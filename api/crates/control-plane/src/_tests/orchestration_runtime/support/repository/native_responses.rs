use super::*;

impl InMemoryOrchestrationRuntimeRepository {
    pub(crate) fn native_recovery_page_reads(&self) -> usize {
        self.inner.lock().unwrap().native_recovery_page_reads
    }

    pub(crate) fn alter_native_recovery_source(
        &self,
        run_id: Uuid,
        mode: &str,
        predecessor: Option<crate::ports::NativeContinuationRecoverySource>,
    ) {
        let mut inner = self.inner.lock().unwrap();
        let events = inner
            .runtime_events_by_flow_run_id
            .get_mut(&run_id)
            .unwrap();
        match mode {
            "missing_output" => {
                events.retain(|event| event.event_type != "provider_output_item_done")
            }
            "missing_reply" => events.retain(|event| {
                !(event.event_type == "provider_semantic_step"
                    && event.payload["kind"] == "model_reply")
            }),
            "duplicate_output" => {
                if let Some(mut duplicate) = events
                    .iter()
                    .find(|event| event.event_type == "provider_output_item_done")
                    .cloned()
                {
                    duplicate.id = Uuid::now_v7();
                    duplicate.sequence =
                        events.iter().map(|event| event.sequence).max().unwrap() + 1;
                    events.push(duplicate);
                }
            }
            "ambiguous" => {
                let originals = events.clone();
                let mut sequence = events.iter().map(|event| event.sequence).max().unwrap();
                for mut event in originals {
                    sequence += 1;
                    event.id = Uuid::now_v7();
                    event.sequence = sequence;
                    events.push(event);
                }
            }
            "proof" => {
                let event = events
                    .iter_mut()
                    .find(|event| event.event_type == "provider_output_item_done")
                    .unwrap();
                event.payload["item"]["future"] = json!({"tampered":true});
            }
            "legacy" | "foreign_scope" | "cycle" => {
                for event in events.iter_mut().filter(|event| {
                    event.event_type == "provider_semantic_step"
                        && event.payload["kind"] == "model_call"
                }) {
                    let mut body: Value =
                        serde_json::from_str(event.payload["body"].as_str().unwrap()).unwrap();
                    match mode {
                        "legacy" => {
                            body.as_object_mut().unwrap().remove("native_context");
                        }
                        "foreign_scope" => {
                            body["native_context"]["workspace_id"] = json!(Uuid::now_v7())
                        }
                        "cycle" => {
                            body["native_context"]["predecessor_source"] =
                                json!({"flow_run_id":run_id,"response_round_id":run_id});
                            body["native_request"]["wire_body"]["previous_response_id"] =
                                json!("upstream-tool-round");
                        }
                        _ => unreachable!(),
                    }
                    event.payload["body"] = json!(body.to_string());
                    if let Some(source) = predecessor {
                        event.payload["context_flow_run_id"] = json!(source.flow_run_id);
                        event.payload["context_response_id"] =
                            json!(format!("resp_{}", source.response_round_id));
                    }
                }
            }
            _ => panic!("unknown source mutation"),
        }
    }

    pub(crate) async fn enable_native_responses_fixture(&self) {
        let (installation_id, manifest_path) = {
            let inner = self.inner.lock().unwrap();
            let instance = inner
                .instances_by_id
                .get(&self.default_provider_instance_id)
                .expect("default fixture provider");
            let artifact = inner
                .artifact_instances_by_key
                .values()
                .find(|artifact| artifact.installation_id == instance.installation_id)
                .unwrap();
            (
                instance.installation_id,
                std::path::Path::new(artifact.local_path.as_ref().unwrap()).join("manifest.yaml"),
            )
        };
        let raw = std::fs::read_to_string(&manifest_path).unwrap();
        let marker = "  entry: bin/fixture_provider-provider\n";
        assert_eq!(
            raw.matches(marker).count(),
            1,
            "fixture runtime entry must be unique"
        );
        let native = raw.replace(marker,
            "  entry: bin/fixture_provider-provider\n  capabilities:\n    - responses.native_passthrough\n    - responses.native_output.v1\n    - native_continuation_supported\n");
        std::fs::write(&manifest_path, native).unwrap();
        let fingerprint = plugin_framework::compute_manifest_fingerprint(&manifest_path)
            .await
            .unwrap();
        let mut inner = self.inner.lock().unwrap();
        inner
            .artifact_instances_by_key
            .values_mut()
            .find(|artifact| artifact.installation_id == installation_id)
            .unwrap()
            .manifest_fingerprint = Some(fingerprint);
    }
}
