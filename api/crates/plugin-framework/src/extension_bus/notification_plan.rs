//! Host-owned managed notification contribution point. Execution uses the existing dispatcher.
use super::*;

pub fn notification_extension_point(
    owner_module_id: ModuleId,
) -> Result<ExtensionPointDescriptor, DescriptorValueError> {
    notification_point(
        owner_module_id,
        extension_contracts::NOTIFICATION_POINT,
        extension_contracts::NOTIFICATION_SEND_PERMISSION,
    )
}

/// Receive and attachment downloads have their own permission and binding; send grants do not
/// authorize polling. Host owns the background lifecycle and durable cursor commit.
pub fn notification_receive_extension_point(
    owner_module_id: ModuleId,
) -> Result<ExtensionPointDescriptor, DescriptorValueError> {
    notification_point(
        owner_module_id,
        extension_contracts::NOTIFICATION_RECEIVE_POINT,
        extension_contracts::NOTIFICATION_RECEIVE_PERMISSION,
    )
}

fn notification_point(
    owner_module_id: ModuleId,
    point: &str,
    permission: &str,
) -> Result<ExtensionPointDescriptor, DescriptorValueError> {
    Ok(ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(point)?,
        owner_module_id,
        point_kind: ExtensionPointKind::ResourceAction,
        contract: ContractDescriptor::new(
            "notification",
            extension_contracts::NOTIFICATION_CONTRACT_V1,
        )?,
        scope: ScopeSemantics::Workspace,
        cardinality: Cardinality::Many,
        ordering: OrderingSemantics::Lexicographic,
        failure: FailureSemantics::FailClosed,
        delivery: DeliverySemantics::Synchronous,
        lifecycle: LifecycleSemantics::Invocation,
        allowed_permissions: [PermissionCode::new(permission)?].into_iter().collect(),
        override_policy: OverridePolicy::Sealed,
    })
}
