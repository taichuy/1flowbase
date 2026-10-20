use super::*;

/// Root #2014: workspace cleanup for a new event never depends on an example contract name.
#[tokio::test]
async fn root_2014_generic_event_history_scope_is_verified_or_conservative() {
    use control_plane_contracts::ports::ManagedLifecycleOutboxRepository;
    let store = store().await;
    let workspace = Uuid::now_v7();
    let event = Uuid::now_v7();
    let transaction = Uuid::now_v7();
    let identity = extension_contracts::ManagedExecutionIdentity::new(
        extension_contracts::ManagedInstallationId::new(Uuid::now_v7().to_string()).unwrap(),
        extension_contracts::ManagedWorkspaceId::new(workspace.to_string()).unwrap(),
        extension_contracts::ContributionId::new("orion.shipments.publish").unwrap(),
        extension_contracts::ManagedArtifactFingerprint::from_bytes(b"artifact"),
        extension_contracts::ManagedBindingFingerprint::from_bytes(b"binding"),
    );
    let fact = extension_contracts::ManagedEventFact {
        event_id: event.to_string(),
        transaction_id: transaction.to_string(),
        contract_id: "orion.shipments.created".into(),
        contract_version: "9".into(),
        workspace_id: workspace.to_string(),
        publisher: identity,
        causation_id: Uuid::now_v7().to_string(),
        correlation_id: Uuid::now_v7().to_string(),
        payload: serde_json::json!({"shipment_id":"s1","destination_code":"NYC","item_count":3}),
    };
    let plan = publication(&["native_observer"]);
    let target = plan.subscribers[0].clone();
    let graph = plan.graph_fingerprint.clone();
    store
        .record_lifecycle_fact(&RecordLifecycleFactInput {
            event_id: event,
            transaction_id: transaction,
            contract_id: fact.contract_id.clone(),
            contract_version: fact.contract_version.clone(),
            canonical_payload: serde_json::to_vec(&fact).unwrap(),
            occurred_at: OffsetDateTime::now_utc(),
            publication: plan,
        })
        .await
        .unwrap();
    assert!(store
        .lifecycle_target_has_backlog(workspace, &graph, &target)
        .await
        .unwrap());
    assert!(!store
        .lifecycle_target_has_backlog(Uuid::now_v7(), &graph, &target)
        .await
        .unwrap());
    sqlx::query("update lifecycle_outbox set canonical_payload=$2 where event_id=$1")
        .bind(event)
        .bind(br#"{"unknown":true}"#.as_slice())
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .lifecycle_target_has_backlog(Uuid::now_v7(), &graph, &target)
        .await
        .unwrap());
}

#[tokio::test]
async fn root_2325_keyset_history_crosses_filtered_pages_and_subscriber_boundary() {
    use control_plane_contracts::ports::*;
    let store = store().await;
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Bounded governance")
        .await
        .unwrap()
        .id;
    let other_workspace = Uuid::now_v7();
    let user = Uuid::now_v7();
    sqlx::query("insert into users(id,account,email,password_hash,name,nickname,status) values($1,$2,$3,'x','Bounded','Bounded','active')")
        .bind(user).bind(format!("bounded-{user}")).bind(format!("bounded-{user}@example.test"))
        .execute(store.pool()).await.unwrap();
    let installation = Uuid::now_v7();
    let contribution = "acme.bounded.events";
    sqlx::query("insert into extension_installations(id,category,organization,artifact_id,artifact_version,plugin_id,contract_version,protocol,display_name,source_kind,trust_level,verification_status,desired_state,signature_status,metadata_json,created_by) values($1,'runtime-extensions','acme','bounded','1.0.0','acme.bounded','1flowbase.extension-bus/v1','stdio_json','Bounded','uploaded','unverified','valid','active_requested','missing',$2,$3)")
        .bind(installation).bind(serde_json::json!({"managed":{"module":{"contributions":[{"contribution_id":contribution}]}}}))
        .bind(user).execute(store.pool()).await.unwrap();
    sqlx::query("insert into plugin_contribution_authorization_revisions(installation_id,workspace_id) values($1,$2)")
        .bind(installation).bind(workspace).execute(store.pool()).await.unwrap();
    let version = format!(
        "{}:sha256:{}:sha256:{}:1:{installation}",
        Uuid::now_v7(),
        "a".repeat(64),
        "b".repeat(64)
    );

    let foreign = Uuid::now_v7();
    // Compact UUID epoch is canonical-parser-valid but outside SQL's conservative owner shape.
    let foreign_version = format!(
        "{}:sha256:{}:sha256:{}:1:{foreign}",
        Uuid::now_v7().simple(),
        "a".repeat(64),
        "b".repeat(64)
    );
    assert_eq!(
        managed_handler_installation(&foreign_version),
        Some(foreign)
    );
    let event = Uuid::from_u128(1);
    sqlx::query("insert into lifecycle_outbox(event_id,transaction_id,contract_id,contract_version,canonical_payload,occurred_at,graph_fingerprint) values($1,$1,'history','v1',decode('ff','hex'),now(),'history')").bind(event).execute(store.pool()).await.unwrap();
    let skipped = (0..256)
        .map(|i| format!("managed.{workspace}.a{i:04}"))
        .collect::<Vec<_>>();
    let expected = (0..600)
        .map(|i| format!("managed.{workspace}.z{i:04}"))
        .collect::<Vec<_>>();
    for (subscribers, handler_version) in [(&skipped, &foreign_version), (&expected, &version)] {
        sqlx::query("insert into lifecycle_outbox_deliveries(event_id,subscriber_id,handler_id,handler_version) select $1,s,s,$3 from unnest($2::text[]) s").bind(event).bind(subscribers).bind(handler_version).execute(store.pool()).await.unwrap();
    }
    let first = store
        .managed_lifecycle_delivery_page(installation, workspace, None)
        .await
        .unwrap();
    assert!(
        first.deliveries.is_empty(),
        "Rust owner filter consumes the whole first candidate page"
    );
    assert!(first.truncated);
    let mut cursor = first.next_cursor.expect("empty filtered page must advance");
    let position = ManagedDeliveryCursor::decode(&cursor, installation, workspace).unwrap();
    assert_eq!(position.subscriber_id, skipped[255]);
    for (installation_id, workspace_id, value) in [
        (other_workspace, workspace, cursor.as_str()),
        (installation, other_workspace, cursor.as_str()),
        (installation, workspace, "invalid"),
    ] {
        assert!(store
            .managed_lifecycle_delivery_page(installation_id, workspace_id, Some(value))
            .await
            .unwrap_err()
            .is::<InvalidManagedDeliveryCursor>());
    }
    let mut observed = Vec::new();
    let mut pages = 0;
    loop {
        let page = store
            .managed_lifecycle_delivery_page(installation, workspace, Some(&cursor))
            .await
            .unwrap();
        pages += 1;
        assert!(pages <= 3, "continuation must strictly progress");
        assert!(page.deliveries.iter().all(|row| row.event_id == event));
        observed.extend(page.deliveries.into_iter().map(|row| row.subscriber_id));
        assert_eq!(page.truncated, page.next_cursor.is_some());
        match page.next_cursor {
            Some(next) => {
                assert_ne!(cursor, next);
                cursor = next;
            }
            None => break,
        }
    }
    assert_eq!(pages, 3);
    assert_eq!(
        observed, expected,
        "complete ordered history has no omission or duplicate"
    );
    // Apply the same completeness oracle to the old fixed-window result and to an early
    // stop on the empty filtered page. Both controlled counterexamples must be rejected.
    let require_complete = |rows: &[String]| assert_eq!(rows, expected.as_slice());
    require_complete(&observed);
    assert!(std::panic::catch_unwind(|| require_complete(&observed[..256])).is_err());
    assert!(std::panic::catch_unwind(|| require_complete(&[])).is_err());
}
