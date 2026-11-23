use super::*;
use extension_contracts::{
    WORKSPACE_POLICY_CONTRACT_ID, WORKSPACE_POLICY_CONTRACT_V1, WORKSPACE_POLICY_PERMISSION,
    WORKSPACE_POLICY_POINT,
};

pub fn workspace_policy_extension_point(
    owner_module_id: ModuleId,
) -> Result<ExtensionPointDescriptor, DescriptorValueError> {
    Ok(ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(WORKSPACE_POLICY_POINT)?,
        owner_module_id,
        point_kind: ExtensionPointKind::ResourceAction,
        contract: ContractDescriptor::new(
            WORKSPACE_POLICY_CONTRACT_ID,
            WORKSPACE_POLICY_CONTRACT_V1,
        )?,
        scope: ScopeSemantics::System,
        cardinality: Cardinality::ZeroOrOne,
        ordering: OrderingSemantics::Lexicographic,
        failure: FailureSemantics::FailClosed,
        delivery: DeliverySemantics::Synchronous,
        lifecycle: LifecycleSemantics::Invocation,
        allowed_permissions: [PermissionCode::new(WORKSPACE_POLICY_PERMISSION)?]
            .into_iter()
            .collect(),
        override_policy: OverridePolicy::Sealed,
    })
}
