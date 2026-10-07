use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEPARTMENT_MODEL_ID: Uuid = Uuid::from_u128(0xde9a0000_0000_4000_8000_000000000001);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Department {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub role_codes: Vec<String>,
    pub member_count: i64,
}

/// A department projected for a lazy tree or a search result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DepartmentTreeItem {
    #[serde(flatten)]
    pub department: Department,
    pub has_children: bool,
    pub is_match: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DepartmentPage {
    pub items: Vec<DepartmentTreeItem>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemberDepartments {
    pub department_ids: Vec<Uuid>,
    pub primary_department_id: Option<Uuid>,
}

impl MemberDepartments {
    pub fn is_valid(&self) -> bool {
        let unique: std::collections::HashSet<_> = self.department_ids.iter().collect();
        unique.len() == self.department_ids.len()
            && if self.department_ids.is_empty() {
                self.primary_department_id.is_none()
            } else {
                self.primary_department_id
                    .is_some_and(|id| self.department_ids.contains(&id))
            }
    }
}

#[cfg(test)]
#[path = "_tests/organization_tests.rs"]
mod tests;
