use super::*;

pub(super) fn validate_decision_binding(
    interface_id: &InterfaceId,
    definition: &InterfaceDefinition,
    registrations: &[(u32, InterfaceExtensionRegistration)],
    point: InterfaceExtensionPoint,
    executable: Option<&Arc<dyn ErasedInterfaceDecisionPlan>>,
    graph: &GraphFingerprint,
) -> Result<(), RegistryCompilationError> {
    let mut registered = registrations
        .iter()
        .filter(|(_, registration)| registration.point() == point)
        .map(|(order, registration)| (*order, registration.plugin().clone()))
        .collect::<Vec<_>>();
    registered.sort();
    let registered = registered
        .into_iter()
        .map(|(_, plugin)| plugin)
        .collect::<Vec<_>>();
    let Some(executable) = executable else {
        if let Some(plugin) = registered.first() {
            return Err(RegistryCompilationError::MissingExecutableExtension(
                interface_id.clone(),
                plugin.clone(),
                point,
            ));
        }
        return Ok(());
    };
    if executable.graph_fingerprint() != graph {
        return Err(RegistryCompilationError::ExtensionGraphFingerprintMismatch(
            interface_id.clone(),
        ));
    }
    if executable.input_contract() != definition.input_contract()
        || executable.output_contract() != definition.output_contract()
    {
        return Err(RegistryCompilationError::DecisionContractMismatch(
            interface_id.clone(),
            point,
        ));
    }
    let bound = executable.plugin_bindings();
    if let Some(plugin) = registered
        .iter()
        .find(|plugin| !bound.contains(plugin))
        .or_else(|| registered.get(bound.len()))
    {
        return Err(RegistryCompilationError::MissingExecutableExtension(
            interface_id.clone(),
            plugin.clone(),
            point,
        ));
    }
    if let Some(plugin) = bound
        .iter()
        .find(|plugin| !registered.contains(plugin))
        .or_else(|| bound.get(registered.len()))
    {
        return Err(RegistryCompilationError::UnexpectedExecutableExtension(
            interface_id.clone(),
            plugin.clone(),
            point,
        ));
    }
    if registered != bound {
        return Err(RegistryCompilationError::MissingExecutableExtension(
            interface_id.clone(),
            registered
                .first()
                .expect("mismatched non-empty decision bindings")
                .clone(),
            point,
        ));
    }
    Ok(())
}
