mod batch;
mod descriptor;
mod filter;

pub use batch::{filter_by_tk_values, ResourceBatchSelection};
pub use descriptor::{ResourceCrudDescriptor, ResourceFilterTarget};
pub use domain::{ResourceFilterExpr, ResourceFilterOperator};
pub use filter::{
    filter_resource_records, matches_resource_filter, parse_resource_filter,
    parse_resource_filter_expr,
};

impl ResourceFilterTarget for domain::ModelDefinitionRecord {
    fn field_value(&self, field: &str) -> Option<String> {
        match field {
            "id" => Some(self.id.to_string()),
            "scope_kind" => Some(self.scope_kind.as_str().to_string()),
            "scope_id" => Some(self.scope_id.to_string()),
            "code" => Some(self.code.clone()),
            "title" => Some(self.title.clone()),
            "status" => Some(self.status.as_str().to_string()),
            "availability_status" => Some(self.availability_status.as_str().to_string()),
            "data_source_instance_id" => self.data_source_instance_id.map(|id| id.to_string()),
            "source_kind" => Some(self.source_kind.as_str().to_string()),
            "external_resource_key" => self.external_resource_key.clone(),
            "external_table_id" => self.external_table_id.clone(),
            "physical_table_name" => Some(self.physical_table_name.clone()),
            _ => None,
        }
    }
}
