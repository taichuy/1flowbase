use control_plane_contracts::ports::{
    ApplicationManagementQuery, ApplicationManagementRepository,
    ApplicationManagementSortDirection, ApplicationManagementSortField, MemberRepository,
    OrchestrationRuntimeRepository,
};
use domain::ResourceFilterExpr;
use uuid::Uuid;

use super::member_role_repository_tests::{bootstrapped_store, create_member};

#[tokio::test]
async fn member_deletion_preserves_history_and_cleans_identity() {
    let (store, workspace, root) = bootstrapped_store().await;
    let member = create_member(&store, workspace, root, "history-owner").await;
    let application = Uuid::now_v7();
    let flow = Uuid::now_v7();
    let draft = Uuid::now_v7();
    let run = Uuid::now_v7();
    let conversation = Uuid::now_v7();
    let key = Uuid::now_v7();
    let public_conversation = Uuid::now_v7();
    let ledger = Uuid::now_v7();

    // This fixture exercises both direct attribution and formerly cascading
    // conversation/credit-account references, using the real migration set.
    for (statement, ids) in [
        ("insert into applications (id,workspace_id,application_type,name,description,created_by,updated_by) values ($1,$2,'agent_flow','Retained history','',$3,$3)", vec![application, workspace, member.id]),
        ("insert into flows (id,application_id,scope_id,created_by,updated_by) values ($1,$2,$3,$4,$4)", vec![flow, application, workspace, member.id]),
        ("insert into flow_drafts (id,flow_id,scope_id,schema_version,document,created_by,updated_by) values ($1,$2,$3,'0.1.0','{}',$4,$4)", vec![draft, flow, workspace, member.id]),
        ("insert into assistant_conversations (conversation_id,scope_id,application_id,created_by) values ($1,$2,$3,$4)", vec![conversation, workspace, application, member.id]),
        ("insert into flow_runs (id,application_id,flow_id,flow_draft_id,scope_id,run_mode,status,created_by,assistant_conversation_id,input_payload,output_payload) values ($1,$2,$3,$4,$5,'debug_flow_run','succeeded',$6,$7,'{\"input\":1}','{\"output\":2}')", vec![run, application, flow, draft, workspace, member.id, conversation]),
        ("insert into node_run_records (id,flow_run_id,scope_id,node_id,node_type,node_alias,status,output_payload) values ($1,$2,$3,'test','test','Test','succeeded','{\"result\":3}')", vec![Uuid::now_v7(), run, workspace]),
        ("insert into api_keys (id,name,token_hash,token_prefix,creator_user_id,tenant_id,scope_kind,scope_id,key_kind,application_id) select $1,'Removed credential','test-member-deletion-key','test',$2,tenant_id,'workspace',id,'application_api_key',$4 from workspaces where id=$3", vec![key, member.id, workspace, application]),
        ("insert into application_public_conversations (id,application_id,api_key_id,external_user,external_conversation_id) values ($1,$2,$3,'external-user','history')", vec![public_conversation, application, key]),
        ("insert into runtime_credit_ledger (id,transaction_id,workspace_id,user_id,account_id,flow_run_id,transaction_type,amount,credit_unit,reason,idempotency_key,status) select $1,$1,$2,$3,id,$4,'charge',1,'USD','history','member-delete-history','settled' from user_credit_accounts where workspace_id=$2 and user_id=$3", vec![ledger, workspace, member.id, run]),
    ] {
        let mut query = sqlx::query(statement);
        for id in ids {
            query = query.bind(id);
        }
        assert_eq!(query.execute(store.pool()).await.unwrap().rows_affected(), 1);
    }

    let before = store.get_flow_run(application, run).await.unwrap().unwrap();
    assert_eq!(before.authorized_account.as_deref(), Some("history-owner"));
    // Root and self-delete protection remain active.
    assert!(store.delete_member(root, root).await.is_err());
    assert!(store.delete_member(member.id, member.id).await.is_err());
    store.delete_member(root, member.id).await.unwrap();

    for table in [
        "users",
        "user_auth_identities",
        "user_role_bindings",
        "workspace_memberships",
        "api_keys",
    ] {
        let column = match table {
            "users" => "id",
            "api_keys" => "creator_user_id",
            _ => "user_id",
        };
        let count: i64 =
            sqlx::query_scalar(&format!("select count(*) from {table} where {column}=$1"))
                .bind(member.id)
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(count, 0, "{table} must be removed");
    }
    let history = store.get_flow_run(application, run).await.unwrap().unwrap();
    assert_eq!(history.created_by, member.id);
    assert!(history.authorized_account.is_none());
    assert_eq!(history.input_payload, before.input_payload);
    assert_eq!(history.output_payload, before.output_payload);
    assert_eq!(history.status, before.status);

    let page = store
        .list_application_management(
            workspace,
            &ApplicationManagementQuery {
                filter: ResourceFilterExpr::All(vec![]),
                sort_field: ApplicationManagementSortField::UpdatedAt,
                sort_direction: ApplicationManagementSortDirection::Desc,
                page: 1,
                page_size: 20,
            },
        )
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, application);
    assert_eq!(page.items[0].created_by, member.id);
    assert_eq!(page.items[0].created_by_display_name, "");

    for (statement, id) in [
        ("select count(*) from assistant_conversations where conversation_id=$1", conversation),
        ("select count(*) from application_public_conversations where id=$1", public_conversation),
        ("select count(*) from user_credit_accounts where user_id=$1", member.id),
        ("select count(*) from runtime_credit_ledger where id=$1 and amount=1 and account_id is not null and user_id is null", ledger),
        (r#"select count(*) from node_run_records where flow_run_id=$1 and output_payload='{"result":3}'::jsonb"#, run),
    ] {
        let count: i64 = sqlx::query_scalar(statement).bind(id).fetch_one(store.pool()).await.unwrap();
        assert_eq!(count, 1, "history must survive: {statement}");
    }

    // Reusing an account name creates a new identity, never reassigns history.
    let replacement = create_member(&store, workspace, root, "history-owner").await;
    assert_ne!(replacement.id, member.id);
    let history = store.get_flow_run(application, run).await.unwrap().unwrap();
    assert_eq!(history.created_by, member.id);
    assert!(history.authorized_account.is_none());
}

#[tokio::test]
async fn member_deletion_schema_has_no_blocking_user_references() {
    let (store, _, _) = bootstrapped_store().await;
    let blocking: Vec<String> = sqlx::query_scalar(
        "select conname from pg_constraint where contype='f' and confrelid='users'::regclass and confdeltype in ('a','r')",
    ).fetch_all(store.pool()).await.unwrap();
    assert!(blocking.is_empty(), "blocking references: {blocking:?}");
    let cascade_tables: Vec<String> = sqlx::query_scalar(
        "select relation.relname::text from pg_constraint c join pg_class relation on relation.oid=c.conrelid where c.contype='f' and c.confrelid='users'::regclass and c.confdeltype='c' order by relation.relname",
    ).fetch_all(store.pool()).await.unwrap();
    assert_eq!(
        cascade_tables,
        [
            "api_keys",
            "data_source_preview_sessions",
            "debug_variable_cache_entries",
            "mcp_client_credentials",
            "model_provider_preview_sessions",
            "user_auth_identities",
            "user_role_bindings",
            "workspace_memberships",
        ]
    );
}
