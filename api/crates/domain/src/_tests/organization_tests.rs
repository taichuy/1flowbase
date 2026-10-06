use super::*;

#[test]
fn organization_requires_one_primary_among_unique_departments() {
    let a = Uuid::now_v7();
    let b = Uuid::now_v7();
    assert!(MemberDepartments::default().is_valid());
    assert!(MemberDepartments {
        department_ids: vec![a, b],
        primary_department_id: Some(a)
    }
    .is_valid());
    assert!(!MemberDepartments {
        department_ids: vec![a],
        primary_department_id: None
    }
    .is_valid());
    assert!(!MemberDepartments {
        department_ids: vec![a],
        primary_department_id: Some(b)
    }
    .is_valid());
    assert!(!MemberDepartments {
        department_ids: vec![],
        primary_department_id: Some(a)
    }
    .is_valid());
    assert!(!MemberDepartments {
        department_ids: vec![a, a],
        primary_department_id: Some(a)
    }
    .is_valid());
}

#[test]
fn organization_builtin_records_are_read_only_tree_fields_are_protected() {
    let contract = crate::builtin_data_model_contract("departments").unwrap();
    assert!(contract.capabilities.record.can_list);
    assert!(contract.capabilities.record.can_get);
    assert!(!contract.capabilities.record.can_create);
    assert!(!contract.capabilities.record.can_update);
    assert!(!contract.capabilities.record.can_delete);
    assert!(!contract.capabilities.can_add_user_field);
    for code in ["parent_id", "tree_partition_id", "sibling_rank", "scope_id"] {
        assert!(contract.owns_field_code(code));
    }
}
