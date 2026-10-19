//! Authentication providers are finite system contributions owned by the Host inventory.
use super::*;

pub fn authentication_extension_point(
    owner_module_id: ModuleId,
) -> Result<ExtensionPointDescriptor, DescriptorValueError> {
    Ok(ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(extension_contracts::AUTHENTICATION_POINT)?,
        owner_module_id,
        point_kind: ExtensionPointKind::ResourceAction,
        contract: ContractDescriptor::new(
            "authentication",
            extension_contracts::AUTHENTICATION_CONTRACT_V1,
        )?,
        scope: ScopeSemantics::System,
        cardinality: Cardinality::Many,
        ordering: OrderingSemantics::Lexicographic,
        failure: FailureSemantics::FailClosed,
        delivery: DeliverySemantics::Synchronous,
        lifecycle: LifecycleSemantics::Invocation,
        allowed_permissions: [
            PermissionCode::new(extension_contracts::AUTHENTICATION_PERMISSION)?,
            PermissionCode::new("network.http.request")?,
        ]
        .into_iter()
        .collect(),
        override_policy: OverridePolicy::Sealed,
    })
}
