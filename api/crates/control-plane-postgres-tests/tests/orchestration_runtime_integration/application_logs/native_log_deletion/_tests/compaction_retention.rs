use super::*;

#[tokio::test]
async fn native_log_deletion_preserves_compaction_owned_shared_contents() {
    let (store, seeded, compiled, actor) = setup().await;
    let at = datetime!(2026-10-07 12:00 UTC);
    let first = run(&store, &seeded, &compiled, true, at).await;
    let second = run(&store, &seeded, &compiled, true, at).await;
    let third = run(&store, &seeded, &compiled, true, at).await;
    let first_body = context(
        &store,
        &seeded,
        first,
        json!({"compaction":"contents owner"}),
    )
    .await;
    let second_body = context(&store, &seeded, second, json!({"compaction":"head owner"})).await;
    let third_body = context(
        &store,
        &seeded,
        third,
        json!({"compaction":"checkpoint only"}),
    )
    .await;
    sqlx::query("insert into runtime_internal_compaction_contents(scope_id,application_id,session_key,content_id,content_kind) values($1,$2,'contents-only',$3,'source')")
        .bind(seeded.workspace_id).bind(seeded.application_id).bind(first_body).execute(store.pool()).await.unwrap();
    sqlx::query("insert into runtime_internal_compaction_heads(scope_id,application_id,session_key,source_content_id,source_hashes,prefix_hash,checkpoint_content_id) values($1,$2,'head-only',$3,array['hash'],'prefix',$4)")
        .bind(seeded.workspace_id).bind(seeded.application_id).bind(second_body).bind(third_body).execute(store.pool()).await.unwrap();
    // Controlled negative: reclaiming a body while ignoring its compaction
    // owner demonstrably cascades away that retained ownership row.
    let mut negative = store.pool().begin().await.unwrap();
    sqlx::query("delete from runtime_context_projections where actual_content_id=$1")
        .bind(first_body)
        .execute(&mut *negative)
        .await
        .unwrap();
    sqlx::query("delete from runtime_canonical_contents where id=$1")
        .bind(first_body)
        .execute(&mut *negative)
        .await
        .unwrap();
    let lost: i64 = sqlx::query_scalar(
        "select count(*) from runtime_internal_compaction_contents where content_id=$1",
    )
    .bind(first_body)
    .fetch_one(&mut *negative)
    .await
    .unwrap();
    assert_eq!(
        lost, 0,
        "controlled negative must demonstrate ownership loss"
    );
    negative.rollback().await.unwrap();
    let service = AgentLogsService::new(store.for_actor(actor.clone()));
    let job = service
        .start_deletion(&actor, seeded.application_id, input())
        .await
        .unwrap();
    assert_eq!(job.total_records, 3);
    store
        .advance_agent_logs_delete_job(job.job_id)
        .await
        .unwrap();
    store
        .advance_agent_logs_delete_job(job.job_id)
        .await
        .unwrap();
    store
        .advance_agent_logs_delete_job(job.job_id)
        .await
        .unwrap();
    assert!(
        !exists(&store, first).await
            && !exists(&store, second).await
            && !exists(&store, third).await
    );
    let bodies: i64 =
        sqlx::query_scalar("select count(*) from runtime_canonical_contents where id=any($1)")
            .bind(vec![first_body, second_body, third_body])
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(bodies, 3);
    let owners: i64 = sqlx::query_scalar(
        "select count(*) from runtime_internal_compaction_contents where content_id=$1",
    )
    .bind(first_body)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(owners, 1, "shared-body ownership must not cascade away");
    let head: (Uuid,Option<Uuid>) = sqlx::query_as("select source_content_id,checkpoint_content_id from runtime_internal_compaction_heads where application_id=$1 and session_key='head-only'")
        .bind(seeded.application_id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(head, (second_body, Some(third_body)));
}
