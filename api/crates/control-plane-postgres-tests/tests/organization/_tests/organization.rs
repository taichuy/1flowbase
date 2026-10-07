use control_plane::{
    organization::OrganizationService,
    ports::{CreateMemberInput, OrganizationRepository, SaveDepartmentInput},
    system_metadata::SystemMetadataBootstrapService,
};
use domain::MemberDepartments;
use uuid::Uuid;

#[tokio::test]
async fn organization_tree_scope_primary_and_delete_invariants() {
    let (store, workspace, user) = super::support::seed_store().await;
    let actor = store
        .load_actor_context(user.id, workspace.tenant_id, workspace.id, None)
        .await
        .unwrap();
    let service = OrganizationService::new(store.for_actor(actor.clone()));
    let root = service
        .save(&actor, None, "Root Department".into(), None, vec![])
        .await
        .unwrap();
    let child = service
        .save(&actor, None, "Child".into(), Some(root.id), vec![])
        .await
        .unwrap();
    let mut parent = child.id;
    for n in 0..12 {
        parent = service
            .save(&actor, None, format!("Depth {n}"), Some(parent), vec![])
            .await
            .unwrap()
            .id;
    }
    let error = service
        .save(&actor, Some(root.id), "Cycle".into(), Some(parent), vec![])
        .await
        .unwrap_err();
    assert!(error.to_string().contains("department_cycle"));
    assert!(service
        .delete(&actor, root.id)
        .await
        .unwrap_err()
        .to_string()
        .contains("department_has_children"));
    let tenant = store.upsert_root_tenant().await.unwrap();
    let other = store
        .upsert_workspace(tenant.id, "Other org workspace")
        .await
        .unwrap();
    assert!(store
        .save_department(&SaveDepartmentInput {
            actor_user_id: user.id,
            workspace_id: other.id,
            department_id: None,
            name: "foreign".into(),
            parent_id: Some(root.id),
            role_codes: vec![],
            can_assign_roles: true
        })
        .await
        .is_err());
    let member = store
        .create_member_with_default_role(&CreateMemberInput {
            actor_user_id: user.id,
            workspace_id: workspace.id,
            account: "organization-member".into(),
            email: "organization@example.com".into(),
            phone: None,
            password_hash: "hash".into(),
            name: "Member".into(),
            nickname: "Member".into(),
            introduction: String::new(),
            email_login_enabled: true,
            phone_login_enabled: false,
        })
        .await
        .unwrap();
    let member_actor = store
        .load_actor_context(member.id, workspace.tenant_id, workspace.id, None)
        .await
        .unwrap();
    let member_service = OrganizationService::new(store.for_actor(member_actor.clone()));
    let access = member_service.access(&member_actor).await.unwrap();
    assert!(
        !access.can_list
            && !access.can_create
            && !access.can_update
            && !access.can_delete
            && !access.can_replace_member_departments
            && !access.can_assign_roles
    );
    assert!(member_service.list(&member_actor).await.is_err());
    assert!(member_service
        .list_page(&member_actor, Default::default())
        .await
        .is_err());
    assert!(member_service
        .replace_member_departments(
            &member_actor,
            member.id,
            MemberDepartments {
                department_ids: vec![child.id],
                primary_department_id: Some(child.id)
            }
        )
        .await
        .is_err());
    for invalid in [
        MemberDepartments {
            department_ids: vec![child.id],
            primary_department_id: None,
        },
        MemberDepartments {
            department_ids: vec![child.id],
            primary_department_id: Some(root.id),
        },
        MemberDepartments {
            department_ids: vec![child.id, child.id],
            primary_department_id: Some(child.id),
        },
    ] {
        assert!(service
            .replace_member_departments(&actor, member.id, invalid)
            .await
            .is_err());
    }
    service
        .replace_member_departments(
            &actor,
            member.id,
            MemberDepartments {
                department_ids: vec![root.id, child.id, parent],
                primary_department_id: Some(child.id),
            },
        )
        .await
        .unwrap();
    // The filtered list is produced by the real repository, without fetching every
    // workspace user then filtering in the HTTP layer. Multiple matching memberships
    // still yield one user, with the same direct roles as the unfiltered projection.
    let all = control_plane::ports::MemberRepository::list_members(&store, workspace.id, None)
        .await
        .unwrap();
    assert!(all.iter().any(|record| record.id == user.id)); // unassigned owner
    let filtered =
        control_plane::ports::MemberRepository::list_members(&store, workspace.id, Some(root.id))
            .await
            .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, member.id);
    assert_eq!(
        filtered[0].roles,
        all.iter()
            .find(|record| record.id == member.id)
            .unwrap()
            .roles
    );
    assert!(
        control_plane::ports::MemberRepository::list_members(&store, other.id, Some(root.id))
            .await
            .is_err()
    );
    assert!(control_plane::ports::MemberRepository::list_members(
        &store,
        workspace.id,
        Some(Uuid::now_v7())
    )
    .await
    .is_err());
    let batched = store
        .members_departments(workspace.id, &[user.id, member.id])
        .await
        .unwrap();
    assert_eq!(batched[&user.id], MemberDepartments::default());
    assert_eq!(
        batched[&member.id],
        store
            .member_departments(workspace.id, member.id)
            .await
            .unwrap()
    );
    assert!(store
        .members_departments(workspace.id, &[])
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        store
            .members_departments(other.id, &[member.id])
            .await
            .unwrap()[&member.id],
        MemberDepartments::default()
    );
    let departments = service.list(&actor).await.unwrap();
    assert_eq!(
        departments
            .iter()
            .find(|d| d.id == root.id)
            .unwrap()
            .member_count,
        1
    );
    assert_eq!(
        store
            .department_member_ids(workspace.id, root.id)
            .await
            .unwrap(),
        vec![member.id]
    );
    assert_eq!(
        store
            .member_departments(workspace.id, member.id)
            .await
            .unwrap()
            .primary_department_id,
        Some(child.id)
    );
    assert!(store
        .replace_member_departments(
            other.id,
            member.id,
            &MemberDepartments {
                department_ids: vec![root.id],
                primary_department_id: Some(root.id)
            }
        )
        .await
        .is_err());
    assert!(service
        .delete(&actor, parent)
        .await
        .unwrap_err()
        .to_string()
        .contains("department_has_members"));
    service
        .replace_member_departments(&actor, member.id, MemberDepartments::default())
        .await
        .unwrap();
    service.delete(&actor, parent).await.unwrap();
    assert_eq!(
        store
            .member_departments(workspace.id, member.id)
            .await
            .unwrap(),
        MemberDepartments::default()
    );
}

#[tokio::test]
async fn organization_role_scope_and_assignment_eligibility() {
    let (store, workspace, user) = super::support::seed_store().await;
    let (code,role_id):(String,Uuid)=sqlx::query_as("select code,id from roles where workspace_id=$1 and scope_kind='workspace' and system_kind is null and code<>'root' limit 1").bind(workspace.id).fetch_one(store.pool()).await.unwrap();
    let mut input = SaveDepartmentInput {
        actor_user_id: user.id,
        workspace_id: workspace.id,
        department_id: None,
        name: "Team".into(),
        parent_id: None,
        role_codes: vec![code.clone()],
        can_assign_roles: false,
    };
    assert!(store
        .save_department(&input)
        .await
        .unwrap_err()
        .to_string()
        .contains("permission_denied"));
    input.can_assign_roles = true;
    let team = store.save_department(&input).await.unwrap();
    assert_eq!(team.role_codes, vec![code.clone()]);
    input.department_id = Some(team.id);
    input.can_assign_roles = false;
    input.name = "Renamed".into();
    assert_eq!(store.save_department(&input).await.unwrap().name, "Renamed");
    input.role_codes.clear();
    assert!(store.save_department(&input).await.is_err());
    input.can_assign_roles = true;
    input.role_codes = vec!["root".into()];
    assert!(store.save_department(&input).await.is_err());
    let other = store
        .upsert_workspace(workspace.tenant_id, "foreign role binding")
        .await
        .unwrap();
    let foreign = store
        .save_department(&SaveDepartmentInput {
            actor_user_id: user.id,
            workspace_id: other.id,
            department_id: None,
            name: "Other".into(),
            parent_id: None,
            role_codes: vec![],
            can_assign_roles: true,
        })
        .await
        .unwrap();
    assert!(sqlx::query("insert into department_role_bindings(id,scope_id,department_id,role_id) values($1,$2,$3,$4)").bind(Uuid::now_v7()).bind(other.id).bind(foreign.id).bind(role_id).execute(store.pool()).await.is_err());
    let direct_count: i64 =
        sqlx::query_scalar("select count(*) from user_role_bindings where user_id=$1")
            .bind(user.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(direct_count > 0);
    // Department roles remain separate from directly assigned user roles.
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from department_role_bindings where department_id=$1"
        )
        .bind(team.id)
        .fetch_one(store.pool())
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn organization_builtin_metadata_is_tree_read_projection() {
    let (store, workspace, user) = super::support::seed_store().await;
    let models = SystemMetadataBootstrapService::new(store.clone())
        .ensure_builtin_user_and_role_models(user.id)
        .await
        .unwrap();
    let model = models
        .iter()
        .find(|m| m.code == "departments")
        .expect("departments registered with users and roles");
    assert_eq!(model.template_code, "ordered_tree");
    assert_eq!(model.physical_table_name, "departments");
    assert!(!domain::data_model_capabilities(model).record.can_update);
    assert!(!domain::data_model_capabilities(model).record.can_create);
    assert!(!domain::data_model_capabilities(model).record.can_delete);
    assert!(model.fields.iter().all(|f| !f.is_writable));
    let grants = SystemMetadataBootstrapService::new(store.clone())
        .ensure_builtin_runtime_read_model_grants(user.id, workspace.id)
        .await
        .unwrap();
    assert!(grants.iter().any(|grant| grant.data_model_id == model.id));
    let metadata = store.list_runtime_model_metadata().await.unwrap();
    let department_metadata = metadata
        .iter()
        .find(|m| m.model_code == "departments")
        .unwrap()
        .clone();
    let actor = store
        .load_actor_context(user.id, workspace.tenant_id, workspace.id, None)
        .await
        .unwrap();
    let department = OrganizationService::new(store.for_actor(actor.clone()))
        .save(&actor, None, "Projected".into(), None, vec![])
        .await
        .unwrap();
    let record = storage_durable::runtime_record_repository::RuntimeRecordRepository::get_record(
        &store,
        &department_metadata,
        Some(workspace.id),
        None,
        &department.id.to_string(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(record["name"], "Projected");
    let registry = runtime_core::runtime_model_registry::RuntimeModelRegistry::default();
    registry.rebuild(metadata);
    let engine =
        runtime_core::runtime_engine::RuntimeEngine::new(registry, std::sync::Arc::new(store));
    let create = engine
        .create_record(runtime_core::runtime_engine::RuntimeCreateInput {
            actor: actor.clone(),
            model_code: "departments".into(),
            payload: serde_json::json!({"name":"Bypass"}),
            scope_grant: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(create.downcast_ref::<storage_durable::runtime_model_availability::RuntimeModelError>(),Some(storage_durable::runtime_model_availability::RuntimeModelError::RecordActionNotAllowed{..})));
    assert!(engine
        .update_record(runtime_core::runtime_engine::RuntimeUpdateInput {
            actor: actor.clone(),
            model_code: "departments".into(),
            record_id: department.id.to_string(),
            payload: serde_json::json!({"name":"Bypass"}),
            scope_grant: None
        })
        .await
        .is_err());
    assert!(engine
        .delete_record(runtime_core::runtime_engine::RuntimeDeleteInput {
            actor,
            model_code: "departments".into(),
            record_id: department.id.to_string(),
            scope_grant: None
        })
        .await
        .is_err());
}

#[tokio::test]
async fn organization_primary_constraint_rejects_native_writer_corruption() {
    let (store, workspace, user) = super::support::seed_store().await;
    let create = |name: &str| SaveDepartmentInput {
        actor_user_id: user.id,
        workspace_id: workspace.id,
        department_id: None,
        name: name.into(),
        parent_id: None,
        role_codes: vec![],
        can_assign_roles: true,
    };
    let a = store.save_department(&create("A")).await.unwrap();
    let b = store.save_department(&create("B")).await.unwrap();
    assert!(sqlx::query("insert into user_department_bindings(id,scope_id,user_id,department_id,is_primary) values($1,$2,$3,$4,false)").bind(Uuid::now_v7()).bind(workspace.id).bind(user.id).bind(a.id).execute(store.pool()).await.is_err());
    store
        .replace_member_departments(
            workspace.id,
            user.id,
            &MemberDepartments {
                department_ids: vec![a.id, b.id],
                primary_department_id: Some(a.id),
            },
        )
        .await
        .unwrap();
    assert!(sqlx::query(
        "delete from user_department_bindings where scope_id=$1 and user_id=$2 and is_primary"
    )
    .bind(workspace.id)
    .bind(user.id)
    .execute(store.pool())
    .await
    .is_err());
    assert_eq!(
        store
            .member_departments(workspace.id, user.id)
            .await
            .unwrap()
            .primary_department_id,
        Some(a.id)
    );
}

#[tokio::test]
async fn organization_pages_preserve_enrichment_selections_and_cursor_boundaries() {
    use control_plane::ports::DepartmentListInput;
    let (store, workspace, user) = super::support::seed_store().await;
    let actor = store
        .load_actor_context(user.id, workspace.tenant_id, workspace.id, None)
        .await
        .unwrap();
    let service = OrganizationService::new(store.for_actor(actor.clone()));
    let root = service
        .save(&actor, None, "Page root".into(), None, vec![])
        .await
        .unwrap();
    let child = service
        .save(&actor, None, "Needle child".into(), Some(root.id), vec![])
        .await
        .unwrap();
    let other = service
        .save(&actor, None, "Page other".into(), None, vec![])
        .await
        .unwrap();
    service
        .replace_member_departments(
            &actor,
            user.id,
            MemberDepartments {
                department_ids: vec![child.id],
                primary_department_id: Some(child.id),
            },
        )
        .await
        .unwrap();
    let first = service
        .list_page(
            &actor,
            DepartmentListInput {
                limit: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].department.id, root.id);
    assert_eq!(first.items[0].department.member_count, 1);
    assert!(first.items[0].has_children && first.items[0].is_match && first.has_more);
    let cursor = first.next_cursor.clone().unwrap();
    let second = service
        .list_page(
            &actor,
            DepartmentListInput {
                limit: 1,
                cursor: Some(cursor.clone()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(second.items[0].department.id, other.id);
    assert!(!second.items[0].has_children);
    assert!(!second.has_more && second.next_cursor.is_none());
    let children = service
        .list_page(
            &actor,
            DepartmentListInput {
                parent_id: Some(root.id),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(children.items[0].department.id, child.id);
    let matches = service
        .list_page(
            &actor,
            DepartmentListInput {
                prefix: Some("Needle".into()),
                limit: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(matches
        .items
        .iter()
        .any(|i| i.department.id == root.id && !i.is_match));
    assert!(matches
        .items
        .iter()
        .any(|i| i.department.id == child.id && i.is_match));
    assert!(service
        .list_page(
            &actor,
            DepartmentListInput {
                parent_id: Some(root.id),
                cursor: Some(cursor.clone()),
                ..Default::default()
            }
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("tree_invalid_cursor"));
    let mut lookup_ids = vec![child.id, root.id, child.id];
    let lookup = service
        .list_page(
            &actor,
            DepartmentListInput {
                ids: Some(lookup_ids.clone()),
                limit: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(lookup.has_more);
    lookup_ids.reverse();
    let lookup_last = service
        .list_page(
            &actor,
            DepartmentListInput {
                ids: Some(lookup_ids),
                limit: 1,
                cursor: lookup.next_cursor,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(!lookup_last.has_more);
    assert_ne!(
        lookup.items[0].department.id,
        lookup_last.items[0].department.id
    );
    assert!(service
        .list_page(
            &actor,
            DepartmentListInput {
                ids: Some(vec![Uuid::new_v4()]),
                ..Default::default()
            }
        )
        .await
        .unwrap()
        .items
        .is_empty());
    // A moved root anchor must not silently resume in its old sibling group.
    service
        .save(
            &actor,
            Some(root.id),
            "Moved root".into(),
            Some(other.id),
            vec![],
        )
        .await
        .unwrap();
    assert!(service
        .list_page(
            &actor,
            DepartmentListInput {
                cursor: Some(cursor),
                ..Default::default()
            }
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("tree_stale_cursor"));
    // A mutation always returns the requested department, even outside the first page.
    let saved = service
        .save(
            &actor,
            Some(child.id),
            "Renamed deep child".into(),
            Some(root.id),
            vec![],
        )
        .await
        .unwrap();
    assert_eq!(saved.id, child.id);
    assert_eq!(saved.name, "Renamed deep child");
}
