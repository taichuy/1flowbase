use super::*;

/// Persisted directory fields used for optimistic concurrency, excluding audit metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpInstanceDirectory {
    pub instance_record_id: Uuid,
    pub groups: Vec<McpDirectoryGroupSnapshot>,
    pub bindings: Vec<McpDirectoryBindingSnapshot>,
}

impl McpInstanceDirectory {
    pub fn canonicalize(&mut self) {
        self.groups.sort_by_key(|group| group.id);
        self.bindings.sort_by_key(|binding| binding.id);
    }

    pub fn from_records(
        instance_record_id: Uuid,
        groups: &[McpGroupRecord],
        bindings: &[McpToolBindingRecord],
    ) -> Self {
        let mut snapshot = Self {
            instance_record_id,
            groups: groups
                .iter()
                .map(|group| McpDirectoryGroupSnapshot {
                    id: group.id,
                    instance_record_id: group.instance_record_id,
                    path: group.path.clone(),
                    display_name: group.display_name.clone(),
                    description_short: group.description_short.clone(),
                    enabled: group.enabled,
                    sort_order: group.sort_order,
                })
                .collect(),
            bindings: bindings
                .iter()
                .map(|binding| McpDirectoryBindingSnapshot {
                    id: binding.id,
                    instance_record_id: binding.instance_record_id,
                    tool_record_id: binding.tool_record_id,
                    tool_id: binding.tool_id.clone(),
                    group_path: binding.group_path.clone(),
                    display_alias: binding.display_alias.clone(),
                    visible: binding.visible,
                    sort_order: binding.sort_order,
                })
                .collect(),
        };
        snapshot.canonicalize();
        snapshot
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpDirectoryGroupSnapshot {
    pub id: Uuid,
    pub instance_record_id: Uuid,
    pub path: String,
    pub display_name: String,
    pub description_short: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpDirectoryBindingSnapshot {
    pub id: Uuid,
    pub instance_record_id: Uuid,
    pub tool_record_id: Uuid,
    pub group_path: String,
    pub tool_id: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpDirectoryGroup {
    pub id: Option<Uuid>,
    pub path: String,
    pub display_name: String,
    pub description_short: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpDirectoryBinding {
    pub id: Option<Uuid>,
    pub tool_id: String,
    pub group_path: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}
