use super::*;
use control_plane_contracts::ports::mcp_oauth::{McpOAuthGrant, McpOAuthRepository};
use serde_json::{json, Value};

const OAUTH_TABLES: [&str; 3] = [
    "mcp_oauth_grants",
    "mcp_oauth_refresh_tokens",
    "mcp_oauth_state",
];

async fn oauth_rows(db: &PgPool) -> BTreeMap<String, Vec<Value>> {
    let mut result = BTreeMap::new();
    for table in OAUTH_TABLES {
        let mut rows = sqlx::query_scalar::<_, Value>(&format!(
            "select to_jsonb(t) from {} t",
            schema::quote(table).unwrap()
        ))
        .fetch_all(db)
        .await
        .unwrap();
        rows.sort_by_key(Value::to_string);
        result.insert(table.into(), rows);
    }
    result
}

#[tokio::test]
async fn selective_mcp_data_roundtrips_oauth_clients_and_lifecycle_facts() {
    let (db, actor) = fixture().await;
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let grant = Uuid::now_v7();
    let client = json!({
        "client_id": "chatgpt_backup_fixture",
        "client_name": "ChatGPT",
        "redirect_uris": ["https://chatgpt.com/connector/oauth/backup-fixture"],
        "token_endpoint_auth_method": "none"
    });
    sqlx::query("insert into mcp_oauth_state(kind,token_hash,payload,expires_at) values('client','client-hash',$1,null),('approval','approval-hash',$2,'2000-01-01T00:00:00Z')")
        .bind(&client).bind(json!({"token_hash":"approval-token-hash"})).execute(&db).await.unwrap();
    let store = crate::PgControlPlaneStore::new(db.clone());
    store
        .oauth_create_grant(&McpOAuthGrant {
            id: grant,
            user_id: actor,
            api_key_id: Uuid::now_v7(),
            key_hash: "key-hash".into(),
            role_code: "member".into(),
            workspace_id: Uuid::now_v7(),
            instance_id: "backup-fixture".into(),
            client_id: "chatgpt_backup_fixture".into(),
            resource: "https://backup.example.com/mcp/backup-fixture".into(),
            expires_at: 946_684_800,
            authorization_stamp: json!({}),
        })
        .await
        .unwrap();
    store.oauth_revoke_grant(grant).await.unwrap();
    sqlx::query("insert into mcp_oauth_refresh_tokens(token_hash,grant_id,expires_at,consumed) values('refresh-hash',$1,'2000-01-01T00:00:00Z',true)")
        .bind(grant).execute(&db).await.unwrap();
    let original = oauth_rows(&db).await;
    assert_eq!(original["mcp_oauth_state"].len(), 2);
    let registration = original["mcp_oauth_state"]
        .iter()
        .find(|row| row["kind"] == "client")
        .unwrap();
    assert_eq!(registration["payload"], client);
    assert!(registration["expires_at"].is_null());
    assert_eq!(original["mcp_oauth_grants"][0]["revoked"], true);
    assert_eq!(original["mcp_oauth_refresh_tokens"][0]["consumed"], true);

    let structure = capture(&repo, select("mcp-management", true, false)).await;
    let scratch = archive::unpack(reader(structure)).await.unwrap();
    let header = archive::header(&mut scratch.reader().await.unwrap())
        .await
        .unwrap();
    assert!(!header
        .tables
        .iter()
        .any(|table| OAUTH_TABLES.contains(&table.name.as_str())));

    let bytes = capture(&repo, select("mcp-management", false, true)).await;
    let scratch = archive::unpack(reader(bytes.clone())).await.unwrap();
    let mut records = scratch.reader().await.unwrap();
    let header = archive::header(&mut records).await.unwrap();
    for table in OAUTH_TABLES {
        let entry = header
            .tables
            .iter()
            .find(|entry| entry.name == table)
            .unwrap();
        assert!(entry.data);
        assert_eq!(entry.feature_id, "system.mcp-management");
    }
    let mut exported: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    while let Some(line) = archive::line(&mut records).await.unwrap() {
        let record: archive::Record = serde_json::from_slice(&line).unwrap();
        if OAUTH_TABLES.contains(&record.table.as_str()) {
            exported.entry(record.table).or_default().push(record.row);
        }
    }
    for rows in exported.values_mut() {
        rows.sort_by_key(Value::to_string);
    }
    assert_eq!(exported, original);

    // Exercise the existing restore, preserving expiry/revocation/rotation state verbatim.
    sqlx::query("delete from mcp_oauth_refresh_tokens")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("delete from mcp_oauth_grants")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("delete from mcp_oauth_state")
        .execute(&db)
        .await
        .unwrap();
    let preview = repo
        .restore(reader(bytes), "key", "key", true)
        .await
        .unwrap();
    assert!(preview.failures.is_empty(), "{:?}", preview.failures);
    assert_eq!(oauth_rows(&db).await, original);
}
