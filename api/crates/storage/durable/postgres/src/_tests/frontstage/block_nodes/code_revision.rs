use super::*;

#[tokio::test]
async fn canonical_source_save_fences_digest_cache_monotonically_and_preserves_cas() {
    let (pool, store, workspace_id, actor_user_id) = block_fixture().await;
    let (page_id, tab_id) =
        create_page_and_tab(&store, workspace_id, actor_user_id, "digest-fence").await;
    store
        .create_frontstage_block_node(&create_input(
            workspace_id,
            actor_user_id,
            page_id,
            tab_id,
            "fenced",
            "fenced-code",
            FrontstageBlockPosition::default(),
            Uuid::now_v7(),
        ))
        .await
        .unwrap();
    // A future prior timestamp rejects now()/clock_timestamp() assignments and
    // proves the fence advances even when wall-clock time has moved backwards.
    sqlx::query("update frontstage_block_nodes set updated_at = clock_timestamp() + interval '1 day' where scope_id = $1 and tree_partition_id = $2 and block_id = 'fenced'")
        .bind(workspace_id).bind(page_id).execute(&pool).await.unwrap();
    let mut prior = store
        .get_frontstage_block_node(workspace_id, page_id, "fenced")
        .await
        .unwrap()
        .unwrap();
    let mut digest = store
        .get_frontstage_block_source_sha256(workspace_id, page_id, "fenced-code")
        .await
        .unwrap()
        .unwrap();
    for source in ["export default 2;", "export default 3;"] {
        let code = store
            .save_frontstage_block_node_code(&SaveFrontstageBlockNodeCodeInput {
                workspace_id,
                actor_user_id,
                page_id,
                block_id: "fenced".into(),
                expected_source_revision: Some(digest.clone()),
                source: FrontstageBlockSourceInput {
                    source_code: source.into(),
                },
                audit_log: audit_log(
                    Some(workspace_id),
                    Some(actor_user_id),
                    "frontstage_block",
                    Some(page_id),
                    "frontstage.block_node_code_saved",
                    json!({"block_id":"fenced"}),
                ),
            })
            .await
            .unwrap();
        let after = store
            .get_frontstage_block_node(workspace_id, page_id, "fenced")
            .await
            .unwrap()
            .unwrap();
        assert!(after.updated_at > prior.updated_at);
        assert_eq!(
            after.updated_at - prior.updated_at,
            time::Duration::microseconds(1)
        );
        assert_ne!(code.source_sha256.as_deref(), Some(digest.as_str()));
        assert_eq!(
            store
                .get_frontstage_block_source_sha256(workspace_id, page_id, "fenced-code")
                .await
                .unwrap(),
            code.source_sha256.clone()
        );
        prior = after;
        digest = code.source_sha256.unwrap();
    }
    let failed = store
        .save_frontstage_block_node_code(&SaveFrontstageBlockNodeCodeInput {
            workspace_id,
            actor_user_id,
            page_id,
            block_id: "fenced".into(),
            expected_source_revision: Some("0".repeat(64)),
            source: FrontstageBlockSourceInput {
                source_code: "must not commit".into(),
            },
            audit_log: audit_log(
                Some(workspace_id),
                Some(actor_user_id),
                "frontstage_block",
                Some(page_id),
                "frontstage.block_node_code_saved",
                json!({"block_id":"fenced"}),
            ),
        })
        .await;
    assert!(failed.is_err());
    assert_eq!(
        store
            .get_frontstage_block_node(workspace_id, page_id, "fenced")
            .await
            .unwrap()
            .unwrap()
            .updated_at,
        prior.updated_at
    );
    assert_eq!(
        store
            .get_frontstage_block_source_sha256(workspace_id, page_id, "fenced-code")
            .await
            .unwrap(),
        Some(digest)
    );
}

#[tokio::test]
async fn legacy_nullable_digest_metadata_preserves_existing_source() {
    let (pool, store, workspace_id, actor_user_id) = block_fixture().await;
    let (page_id, tab_id) =
        create_page_and_tab(&store, workspace_id, actor_user_id, "legacy-digest").await;
    store
        .create_frontstage_block_node(&create_input(
            workspace_id,
            actor_user_id,
            page_id,
            tab_id,
            "legacy",
            "legacy-code",
            FrontstageBlockPosition::default(),
            Uuid::now_v7(),
        ))
        .await
        .unwrap();
    sqlx::query("update frontstage_block_codes set source_sha256 = null where workspace_id = $1 and page_id = $2 and code_ref = 'legacy-code'")
        .bind(workspace_id).bind(page_id).execute(&pool).await.unwrap();
    assert_eq!(
        store
            .get_frontstage_block_source_sha256(workspace_id, page_id, "legacy-code")
            .await
            .unwrap(),
        None
    );
    let code = store
        .get_frontstage_block_code(workspace_id, page_id, "legacy-code")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(code.source_sha256, None);
    assert!(!code.source_code.is_empty());
    assert_eq!(
        store
            .get_frontstage_block_source_sha256(workspace_id, page_id, "absent-code")
            .await
            .unwrap(),
        None
    );
    assert!(store
        .get_frontstage_block_code(workspace_id, page_id, "absent-code")
        .await
        .unwrap()
        .is_none());
}
