//! Host-owned canonical lifecycle declarations derived from the compiled interface inventory.
use super::*;

pub const MANAGED_INTERFACE_MODULE: &str = "1flowbase.interface-lifecycle";
pub const MANAGED_INTERFACE_PHASES: [HookPhase; 6] = [
    HookPhase::Authorization,
    HookPhase::Admission,
    HookPhase::Before,
    HookPhase::After,
    HookPhase::Failure,
    HookPhase::Completion,
];

pub fn managed_interface_permission(phase: HookPhase) -> &'static str {
    match phase {
        HookPhase::Authorization => "hook.interface.authorization",
        HookPhase::Admission => "hook.interface.admission",
        HookPhase::Before => "hook.interface.before",
        HookPhase::After => "hook.interface.after",
        HookPhase::Failure => "hook.interface.failure",
        HookPhase::Completion => "hook.interface.completion",
    }
}

pub fn managed_interface_point_phase(point: &str) -> Option<HookPhase> {
    let target = point.strip_prefix("1flowbase.interface.")?;
    let (interface, suffix) = target.rsplit_once('.')?;
    if interface.is_empty()
        || interface.len() > 384
        || !interface
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return None;
    }
    MANAGED_INTERFACE_PHASES
        .into_iter()
        .find(|phase| managed_interface_permission(*phase).rsplit('.').next() == Some(suffix))
}

pub fn compile_managed_interface_module<'a>(
    interfaces: impl IntoIterator<Item = &'a str>,
) -> Result<ModuleDescriptor, DescriptorValueError> {
    let module_id = ModuleId::new(MANAGED_INTERFACE_MODULE)?;
    let mut points = Vec::new();
    for interface in interfaces {
        for phase in MANAGED_INTERFACE_PHASES {
            points.push(ExtensionPointDescriptor {
                point_id: ExtensionPointId::new(
                    extension_contracts::managed_interface_hook_point_id(interface, phase),
                )?,
                owner_module_id: module_id.clone(),
                point_kind: ExtensionPointKind::Pipeline,
                contract: ContractDescriptor::new("managed-interface", "1")?,
                scope: ScopeSemantics::Workspace,
                cardinality: Cardinality::Many,
                ordering: OrderingSemantics::Dependency,
                failure: if matches!(
                    phase,
                    HookPhase::After | HookPhase::Failure | HookPhase::Completion
                ) {
                    FailureSemantics::BestEffort
                } else {
                    FailureSemantics::FailClosed
                },
                delivery: DeliverySemantics::Synchronous,
                lifecycle: LifecycleSemantics::Invocation,
                allowed_permissions: [PermissionCode::new(managed_interface_permission(phase))?]
                    .into_iter()
                    .collect(),
                override_policy: OverridePolicy::Sealed,
            });
        }
    }
    points.sort_by(|a, b| a.point_id.cmp(&b.point_id));
    Ok(ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id,
        module_version: ModuleVersion::new("1")?,
        module_kind: ModuleKind::BootCore,
        activation: ModuleActivationDeclaration::Active,
        dependencies: Default::default(),
        granted_permissions: Default::default(),
        extension_points: points,
        contributions: Vec::new(),
    })
}
