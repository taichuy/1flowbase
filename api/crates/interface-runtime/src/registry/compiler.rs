use super::*;

impl RegistryCompiler {
    fn has_interface_input(&self, interface_id: &InterfaceId) -> bool {
        self.definitions.contains_key(interface_id)
            || self
                .definition_contributions
                .iter()
                .any(|(_, _, binding)| binding.definition().interface_id() == interface_id)
    }

    pub fn new(
        graph_fingerprint: GraphFingerprint,
        known_operations: impl IntoIterator<Item = AuthorizationOperation>,
        active_owners: impl IntoIterator<Item = InterfaceOwner>,
    ) -> Self {
        Self {
            managed_factory: None,
            graph_fingerprint,
            known_operations: known_operations.into_iter().collect(),
            active_owners: active_owners.into_iter().collect(),
            definition_contributions: Vec::new(),
            compiled_definition_contributions: BTreeMap::new(),
            definitions: BTreeMap::new(),
            protocol_bindings: BTreeMap::new(),
            routes: BTreeMap::new(),
            handler_bindings: BTreeMap::new(),
            extension_handler_bindings: BTreeMap::new(),
            hook_bindings: BTreeMap::new(),
            authorization_bindings: BTreeMap::new(),
            admission_bindings: BTreeMap::new(),
            extensions: BTreeMap::new(),
            authentication_activations: BTreeMap::new(),
        }
    }

    pub fn register_definition_contribution<I, O, E, P>(
        &mut self,
        order: u32,
        registration: InterfaceExtensionRegistration,
        contribution: Arc<TypedInterfaceDefinitionContribution<I, O, E, P>>,
    ) where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
        P: InvocationPrincipal,
    {
        self.definition_contributions
            .push((order, registration, contribution));
    }

    pub fn register_stream_definition_contribution<I, S, O, E, P>(
        &mut self,
        order: u32,
        registration: InterfaceExtensionRegistration,
        contribution: Arc<TypedInterfaceStreamDefinitionContribution<I, S, O, E, P>>,
    ) where
        I: InterfaceContract,
        S: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
        P: InvocationPrincipal,
    {
        self.definition_contributions
            .push((order, registration, contribution));
    }

    pub fn bind_authentication_activation(
        &mut self,
        interface_id: InterfaceId,
        activation: ActivatedAuthenticationAdapter,
    ) -> Result<(), RegistryCompilationError> {
        let key = (interface_id.clone(), activation.adapter().clone());
        if self.authentication_activations.contains_key(&key) {
            return Err(RegistryCompilationError::DuplicateAuthenticationActivation(
                interface_id,
            ));
        }
        self.authentication_activations.insert(key, activation);
        Ok(())
    }

    pub fn register_authentication_adapter(
        &mut self,
        interface_id: &InterfaceId,
        order: u32,
        registration: InterfaceExtensionRegistration,
        activation: ActivatedAuthenticationAdapter,
    ) -> Result<(), RegistryCompilationError> {
        self.register_extension(interface_id, order, registration)?;
        self.bind_authentication_activation(interface_id.clone(), activation)
    }

    pub fn register_definition(
        &mut self,
        definition: InterfaceDefinition,
    ) -> Result<(), RegistryCompilationError> {
        if self.definitions.contains_key(definition.interface_id()) {
            return Err(RegistryCompilationError::DuplicateInterface(
                definition.interface_id().clone(),
            ));
        }
        self.definitions
            .insert(definition.interface_id().clone(), definition);
        Ok(())
    }

    pub fn absorb_snapshot(
        &mut self,
        snapshot: &CompiledInterfaceRegistry,
    ) -> Result<(), RegistryCompilationError> {
        for interface_id in snapshot.definitions.keys() {
            self.absorb_interface(snapshot, interface_id)?;
        }
        Ok(())
    }

    pub fn absorb_interface(
        &mut self,
        snapshot: &CompiledInterfaceRegistry,
        interface_id: &InterfaceId,
    ) -> Result<(), RegistryCompilationError> {
        let definition = snapshot
            .definitions
            .get(interface_id)
            .ok_or_else(|| RegistryCompilationError::UnknownInterface(interface_id.clone()))?;
        self.register_definition(definition.clone())?;
        if let Some(plugin) = snapshot.definition_contributions.get(interface_id) {
            self.compiled_definition_contributions
                .insert(interface_id.clone(), plugin.clone());
        }
        for (binding_id, binding) in snapshot
            .protocol_bindings
            .iter()
            .filter(|(_, binding)| binding.interface_identity().interface_id() == interface_id)
        {
            let adapter_plan = snapshot
                .plans
                .get(binding_id)
                .expect("compiled binding must own an invocation plan")
                .adapter_plan()
                .clone();
            self.register_binding(binding.clone(), adapter_plan)?;
            let authentication = snapshot
                .plans
                .get(binding_id)
                .expect("compiled binding must own an invocation plan")
                .authentication()
                .clone();
            self.authentication_activations.insert(
                (interface_id.clone(), authentication.adapter().clone()),
                authentication,
            );
        }
        let plan = snapshot
            .plan_for_interface(interface_id)
            .ok_or_else(|| RegistryCompilationError::MissingBinding(interface_id.clone()))?;
        let handler = snapshot
            .handler_bindings
            .get(interface_id)
            .ok_or_else(|| RegistryCompilationError::MissingHandler(interface_id.clone()))?;
        if self
            .handler_bindings
            .insert(interface_id.clone(), Arc::clone(handler))
            .is_some()
        {
            return Err(RegistryCompilationError::DuplicateHandler(
                interface_id.clone(),
            ));
        }
        if let Some(hooks) = &plan.executable_extensions {
            self.hook_bindings
                .insert(interface_id.clone(), Arc::clone(hooks));
        }
        if let Some(authorization) = &plan.authorization_extensions {
            self.authorization_bindings
                .insert(interface_id.clone(), Arc::clone(authorization));
        }
        if let Some(admission) = &plan.admission_extensions {
            self.admission_bindings
                .insert(interface_id.clone(), Arc::clone(admission));
        }
        if plan.effective_handler().plugin().as_str() != "builtin.interface-handler" {
            self.extension_handler_bindings
                .entry(interface_id.clone())
                .or_default()
                .insert(
                    plan.effective_handler().plugin().clone(),
                    ContributedInterfaceHandlerBinding {
                        plugin: plan.effective_handler().plugin().clone(),
                        target: plan.effective_handler().target().clone(),
                        binding: Arc::clone(handler),
                    },
                );
        }
        for entry in plan.extension_plan().registrations() {
            self.register_extension(interface_id, entry.order(), entry.registration().clone())?;
        }
        Ok(())
    }

    pub fn register_binding(
        &mut self,
        binding: ProtocolBinding,
        adapter_plan: InvocationAdapterPlan,
    ) -> Result<(), RegistryCompilationError> {
        if self.protocol_bindings.contains_key(binding.binding_id()) {
            return Err(RegistryCompilationError::DuplicateBinding(
                binding.binding_id().clone(),
            ));
        }
        if let ProtocolProjection::Http(route) = binding.projection() {
            if let Some(existing) = self.routes.get(route) {
                return Err(RegistryCompilationError::DuplicateProjection(
                    existing.clone(),
                ));
            }
            self.routes
                .insert(route.clone(), binding.binding_id().clone());
        }
        self.protocol_bindings
            .insert(binding.binding_id().clone(), (binding, adapter_plan));
        Ok(())
    }

    pub fn register_extension(
        &mut self,
        interface_id: &InterfaceId,
        order: u32,
        registration: InterfaceExtensionRegistration,
    ) -> Result<(), RegistryCompilationError> {
        if !self.has_interface_input(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        self.extensions
            .entry(interface_id.clone())
            .or_default()
            .push((order, registration));
        Ok(())
    }

    pub fn bind_hook_plan<I, O>(
        &mut self,
        interface_id: &InterfaceId,
        hooks: Arc<TypedInterfaceHookPlan<I, O>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        O: InterfaceContract,
    {
        if !self.has_interface_input(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        if self.hook_bindings.contains_key(interface_id) {
            return Err(RegistryCompilationError::DuplicateHookPlan(
                interface_id.clone(),
            ));
        }
        self.hook_bindings.insert(interface_id.clone(), hooks);
        Ok(())
    }

    pub fn bind_authorization_plan<I, O>(
        &mut self,
        interface_id: &InterfaceId,
        plan: Arc<TypedInterfaceAuthorizationPlan<I, O>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        O: InterfaceContract,
    {
        if !self.has_interface_input(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        if self
            .authorization_bindings
            .insert(interface_id.clone(), plan)
            .is_some()
        {
            return Err(RegistryCompilationError::DuplicateHookPlan(
                interface_id.clone(),
            ));
        }
        Ok(())
    }

    pub fn bind_admission_plan<I, O>(
        &mut self,
        interface_id: &InterfaceId,
        plan: Arc<TypedInterfaceAdmissionPlan<I, O>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        O: InterfaceContract,
    {
        if !self.has_interface_input(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        if self
            .admission_bindings
            .insert(interface_id.clone(), plan)
            .is_some()
        {
            return Err(RegistryCompilationError::DuplicateHookPlan(
                interface_id.clone(),
            ));
        }
        Ok(())
    }

    pub fn bind_extension_handler<I, O, E, P>(
        &mut self,
        interface_id: &InterfaceId,
        plugin: PluginIdentity,
        handler_reference: HandlerReference,
        target: TargetReference,
        handler: Arc<dyn InterfaceHandler<I, O, E, P>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
        P: InvocationPrincipal,
    {
        let binding: Arc<dyn ErasedInterfaceBinding> =
            Arc::new(TypedInterfaceBinding::<I, O, E, P> {
                contracts: InterfaceContracts::unary(
                    contract_identity::<I>(),
                    contract_identity::<O>(),
                    contract_identity::<E>(),
                ),
                handler_reference,
                handler,
            });
        self.insert_extension_handler(interface_id, plugin, target, binding)
    }

    pub fn bind_extension_stream_handler<I, S, O, E, P>(
        &mut self,
        interface_id: &InterfaceId,
        plugin: PluginIdentity,
        handler_reference: HandlerReference,
        target: TargetReference,
        handler: Arc<dyn InterfaceStreamHandler<I, S, O, E, P>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        S: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
        P: InvocationPrincipal,
    {
        let binding: Arc<dyn ErasedInterfaceBinding> =
            Arc::new(TypedInterfaceStreamBinding::<I, S, O, E, P> {
                contracts: InterfaceContracts::server_stream(
                    contract_identity::<I>(),
                    contract_identity::<S>(),
                    contract_identity::<O>(),
                    contract_identity::<E>(),
                ),
                handler_reference,
                handler,
            });
        self.insert_extension_handler(interface_id, plugin, target, binding)
    }

    fn insert_extension_handler(
        &mut self,
        interface_id: &InterfaceId,
        plugin: PluginIdentity,
        target: TargetReference,
        binding: Arc<dyn ErasedInterfaceBinding>,
    ) -> Result<(), RegistryCompilationError> {
        if !self.has_interface_input(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        let bindings = self
            .extension_handler_bindings
            .entry(interface_id.clone())
            .or_default();
        if bindings.contains_key(&plugin) {
            return Err(RegistryCompilationError::DuplicateExtensionHandler(
                interface_id.clone(),
                plugin,
            ));
        }
        bindings.insert(
            plugin.clone(),
            ContributedInterfaceHandlerBinding {
                plugin,
                target,
                binding,
            },
        );
        Ok(())
    }

    pub fn bind_handler<I, O, E, P>(
        &mut self,
        interface_id: &InterfaceId,
        handler_reference: HandlerReference,
        handler: Arc<dyn InterfaceHandler<I, O, E, P>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
        P: InvocationPrincipal,
    {
        if !self.has_interface_input(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        if self.handler_bindings.contains_key(interface_id) {
            return Err(RegistryCompilationError::DuplicateHandler(
                interface_id.clone(),
            ));
        }
        let contracts = InterfaceContracts::unary(
            contract_identity::<I>(),
            contract_identity::<O>(),
            contract_identity::<E>(),
        );
        self.handler_bindings.insert(
            interface_id.clone(),
            Arc::new(TypedInterfaceBinding::<I, O, E, P> {
                contracts,
                handler_reference,
                handler,
            }),
        );
        Ok(())
    }

    pub fn bind_stream_handler<I, S, O, E, P>(
        &mut self,
        interface_id: &InterfaceId,
        handler_reference: HandlerReference,
        handler: Arc<dyn InterfaceStreamHandler<I, S, O, E, P>>,
    ) -> Result<(), RegistryCompilationError>
    where
        I: InterfaceContract,
        S: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
        P: InvocationPrincipal,
    {
        if !self.has_interface_input(interface_id) {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        if self.handler_bindings.contains_key(interface_id) {
            return Err(RegistryCompilationError::DuplicateHandler(
                interface_id.clone(),
            ));
        }
        let contracts = InterfaceContracts::server_stream(
            contract_identity::<I>(),
            contract_identity::<S>(),
            contract_identity::<O>(),
            contract_identity::<E>(),
        );
        self.handler_bindings.insert(
            interface_id.clone(),
            Arc::new(TypedInterfaceStreamBinding::<I, S, O, E, P> {
                contracts,
                handler_reference,
                handler,
            }),
        );
        Ok(())
    }

    pub fn compile(mut self) -> Result<Arc<CompiledInterfaceRegistry>, RegistryCompilationError> {
        for (order, registration, contribution) in
            std::mem::take(&mut self.definition_contributions)
        {
            let definition = contribution.definition().clone();
            let interface_id = definition.interface_id().clone();
            if registration.point() != InterfaceExtensionPoint::Definition {
                return Err(RegistryCompilationError::UnexpectedExecutableExtension(
                    interface_id,
                    registration.plugin().clone(),
                    registration.point(),
                ));
            }
            self.register_definition(definition)?;
            for binding in contribution.bindings() {
                self.register_binding(binding.binding().clone(), binding.adapter_plan().clone())?;
            }
            self.compiled_definition_contributions
                .insert(interface_id.clone(), registration.plugin().clone());
            self.extensions
                .entry(interface_id)
                .or_default()
                .push((order, registration));
        }
        for (interface_id, definition) in &self.definitions {
            if !self
                .known_operations
                .contains(definition.authorization_operation())
            {
                return Err(RegistryCompilationError::UnknownAuthorizationOperation(
                    interface_id.clone(),
                ));
            }
            if !self.active_owners.contains(definition.owner()) {
                return Err(RegistryCompilationError::InactiveOwner(
                    interface_id.clone(),
                ));
            }
            if definition.execution_mode() != definition.contracts.mode() {
                return Err(RegistryCompilationError::ExecutionModeMismatch(
                    interface_id.clone(),
                ));
            }
            let has_contributed_handler =
                self.extensions
                    .get(interface_id)
                    .is_some_and(|registrations| {
                        registrations.iter().any(|(_, registration)| {
                            registration.point() == InterfaceExtensionPoint::Handler
                        })
                    });
            if !has_contributed_handler {
                let binding = self.handler_bindings.get(interface_id).ok_or_else(|| {
                    RegistryCompilationError::MissingHandler(interface_id.clone())
                })?;
                if binding.contracts() != definition.contracts() {
                    return Err(RegistryCompilationError::ContractMismatch(
                        interface_id.clone(),
                    ));
                }
                if binding.handler_reference() != definition.handler_reference() {
                    return Err(RegistryCompilationError::HandlerReferenceMismatch(
                        interface_id.clone(),
                    ));
                }
                if binding.principal_profile() != definition.principal_profile() {
                    return Err(RegistryCompilationError::PrincipalProfileMismatch(
                        interface_id.clone(),
                    ));
                }
            }
            if !self
                .protocol_bindings
                .values()
                .any(|(binding, _)| binding.interface_identity().interface_id() == interface_id)
            {
                return Err(RegistryCompilationError::MissingBinding(
                    interface_id.clone(),
                ));
            }
            let registrations = self
                .extensions
                .get(interface_id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let definition_registration = registrations
                .iter()
                .find(|(_, registration)| {
                    registration.point() == InterfaceExtensionPoint::Definition
                })
                .map(|(_, registration)| registration);
            match (
                definition_registration,
                self.compiled_definition_contributions.get(interface_id),
            ) {
                (Some(registration), Some(plugin)) if registration.plugin() == plugin => {}
                (Some(_), None) => {
                    return Err(RegistryCompilationError::MissingDefinitionContribution(
                        interface_id.clone(),
                    ));
                }
                (None, Some(_)) => {
                    return Err(RegistryCompilationError::UnexpectedDefinitionContribution(
                        interface_id.clone(),
                    ));
                }
                (Some(_), Some(_)) => {
                    return Err(RegistryCompilationError::UnexpectedDefinitionContribution(
                        interface_id.clone(),
                    ));
                }
                (None, None) => {}
            }
            let authentication_registrations = registrations
                .iter()
                .filter(|(_, registration)| {
                    registration.point() == InterfaceExtensionPoint::AuthenticationAdapter
                })
                .map(|(_, registration)| registration)
                .collect::<Vec<_>>();
            if authentication_registrations.len() != 1 {
                return Err(RegistryCompilationError::MissingAuthenticationActivation(
                    interface_id.clone(),
                ));
            }
            let registration = authentication_registrations[0];
            let referenced_adapters = self
                .protocol_bindings
                .values()
                .filter(|(binding, _)| binding.interface_identity().interface_id() == interface_id)
                .map(|(_, plan)| plan.authentication())
                .collect::<BTreeSet<_>>();
            for adapter in &referenced_adapters {
                let activation = self
                    .authentication_activations
                    .get(&(interface_id.clone(), (*adapter).clone()))
                    .ok_or_else(|| {
                        RegistryCompilationError::MissingAuthenticationActivation(
                            interface_id.clone(),
                        )
                    })?;
                if activation.plugin() != registration.plugin()
                    || activation.tier() != registration.tier()
                    || activation.principal_profile() != definition.principal_profile()
                {
                    return Err(RegistryCompilationError::AuthenticationActivationMismatch(
                        interface_id.clone(),
                    ));
                }
            }
            if self
                .authentication_activations
                .keys()
                .any(|(candidate, adapter)| {
                    candidate == interface_id && !referenced_adapters.contains(adapter)
                })
            {
                return Err(
                    RegistryCompilationError::UnexpectedAuthenticationActivation(
                        interface_id.clone(),
                    ),
                );
            }
        }
        if let Some(interface_id) = self
            .handler_bindings
            .keys()
            .find(|interface_id| !self.definitions.contains_key(*interface_id))
        {
            return Err(RegistryCompilationError::UnknownInterface(
                interface_id.clone(),
            ));
        }
        let mut effective_handler_bindings = BTreeMap::new();
        let mut effective_handlers = BTreeMap::new();
        for (interface_id, definition) in &self.definitions {
            let registrations = self
                .extensions
                .get(interface_id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let handler_plugins = registrations
                .iter()
                .filter(|(_, registration)| {
                    registration.point() == InterfaceExtensionPoint::Handler
                })
                .map(|(_, registration)| registration.plugin().clone())
                .collect::<Vec<_>>();
            let contributed = self.extension_handler_bindings.get(interface_id);
            let (effective, binding) = if handler_plugins.is_empty() {
                if let Some((plugin, _)) = contributed.and_then(|bindings| bindings.iter().next()) {
                    return Err(RegistryCompilationError::UnexpectedExecutableExtension(
                        interface_id.clone(),
                        plugin.clone(),
                        InterfaceExtensionPoint::Handler,
                    ));
                }
                let binding = self
                    .handler_bindings
                    .get(interface_id)
                    .expect("definition handler was validated above");
                (
                    InterfaceHandlerCandidate::new(
                        PluginIdentity::new("builtin.interface-handler")
                            .expect("built-in handler identity must be valid"),
                        binding.handler_reference().clone(),
                        definition.target_reference().clone(),
                    ),
                    Arc::clone(binding),
                )
            } else {
                let mut candidates = Vec::with_capacity(handler_plugins.len());
                for plugin in &handler_plugins {
                    let executable = contributed
                        .and_then(|bindings| bindings.get(plugin))
                        .ok_or_else(|| {
                            RegistryCompilationError::MissingExecutableExtension(
                                interface_id.clone(),
                                plugin.clone(),
                                InterfaceExtensionPoint::Handler,
                            )
                        })?;
                    if executable.binding.contracts() != definition.contracts() {
                        return Err(RegistryCompilationError::ContractMismatch(
                            interface_id.clone(),
                        ));
                    }
                    if executable.binding.principal_profile() != definition.principal_profile() {
                        return Err(RegistryCompilationError::PrincipalProfileMismatch(
                            interface_id.clone(),
                        ));
                    }
                    candidates.push(InterfaceHandlerCandidate::new(
                        executable.plugin.clone(),
                        executable.binding.handler_reference().clone(),
                        executable.target.clone(),
                    ));
                }
                if let Some((plugin, _)) = contributed.and_then(|bindings| {
                    bindings
                        .iter()
                        .find(|(plugin, _)| !handler_plugins.contains(plugin))
                }) {
                    return Err(RegistryCompilationError::UnexpectedExecutableExtension(
                        interface_id.clone(),
                        plugin.clone(),
                        InterfaceExtensionPoint::Handler,
                    ));
                }
                let effective = compile_effective_handler(interface_id, candidates)?;
                let executable = contributed
                    .and_then(|bindings| bindings.get(effective.plugin()))
                    .expect("effective contributed handler was validated above");
                (effective, Arc::clone(&executable.binding))
            };
            effective_handlers.insert(interface_id.clone(), effective);
            effective_handler_bindings.insert(interface_id.clone(), binding);
        }

        let mut managed_contracts = BTreeMap::new();
        // Exact original bytes preserve insertion order in registry fingerprints.
        // This assembly-only index releases its keys after publication.
        let mut schemas = BTreeMap::<Vec<u8>, Arc<serde_json::Value>>::new();
        for (id, binding) in &effective_handler_bindings {
            for mut descriptor in binding.managed_contracts() {
                if let Some(schema) = &mut descriptor.schema {
                    *schema = intern_schema(&mut schemas, schema.clone());
                }
                if self.managed_factory.is_some() && descriptor.schema.is_none() {
                    return Err(RegistryCompilationError::ManagedProjectionMismatch(
                        id.clone(),
                    ));
                }
                if let Some(previous) =
                    managed_contracts.insert(descriptor.contract.clone(), descriptor.clone())
                {
                    if self.managed_factory.is_some() && previous != descriptor {
                        return Err(RegistryCompilationError::ManagedProjectionMismatch(
                            id.clone(),
                        ));
                    }
                }
            }
            if self.managed_factory.is_some() {
                binding.install_managed(&mut self, id)?;
            }
        }

        for interface_id in self.definitions.keys() {
            let registrations = self
                .extensions
                .get(interface_id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let hooks = self.hook_bindings.get(interface_id);
            if let Some(hooks) = hooks {
                if hooks.graph_fingerprint() != &self.graph_fingerprint {
                    return Err(RegistryCompilationError::ExtensionGraphFingerprintMismatch(
                        interface_id.clone(),
                    ));
                }
                let definition = self
                    .definitions
                    .get(interface_id)
                    .expect("hook binding interface definition was registered");
                if hooks.input_contract() != definition.input_contract()
                    || hooks.output_contract() != definition.output_contract()
                {
                    return Err(RegistryCompilationError::HookContractMismatch(
                        interface_id.clone(),
                    ));
                }
                for point in [
                    InterfaceExtensionPoint::Before,
                    InterfaceExtensionPoint::After,
                    InterfaceExtensionPoint::Failure,
                    InterfaceExtensionPoint::Completion,
                ] {
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
                    let bound = hooks
                        .bindings()
                        .iter()
                        .filter(|(_, bound_point)| *bound_point == point)
                        .map(|(plugin, _)| plugin.clone())
                        .collect::<Vec<_>>();
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
                        let plugin = registered
                            .iter()
                            .zip(&bound)
                            .find_map(|(registered, bound)| {
                                (registered != bound).then(|| registered.clone())
                            })
                            .expect("different hook order must have a mismatched entry");
                        return Err(RegistryCompilationError::MissingExecutableExtension(
                            interface_id.clone(),
                            plugin,
                            point,
                        ));
                    }
                }
            } else if let Some((_, registration)) =
                registrations.iter().find(|(_, registration)| {
                    matches!(
                        registration.point(),
                        InterfaceExtensionPoint::Before
                            | InterfaceExtensionPoint::After
                            | InterfaceExtensionPoint::Failure
                            | InterfaceExtensionPoint::Completion
                    )
                })
            {
                return Err(RegistryCompilationError::MissingExecutableExtension(
                    interface_id.clone(),
                    registration.plugin().clone(),
                    registration.point(),
                ));
            }
        }
        for (interface_id, definition) in &self.definitions {
            let registrations = self
                .extensions
                .get(interface_id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            validate_decision_binding(
                interface_id,
                definition,
                registrations,
                InterfaceExtensionPoint::Authorization,
                self.authorization_bindings.get(interface_id),
                &self.graph_fingerprint,
            )?;
            validate_decision_binding(
                interface_id,
                definition,
                registrations,
                InterfaceExtensionPoint::Admission,
                self.admission_bindings.get(interface_id),
                &self.graph_fingerprint,
            )?;
        }
        let mut plans = BTreeMap::new();
        for (binding, adapter_plan) in self.protocol_bindings.values() {
            let Some(definition) = self
                .definitions
                .get(binding.interface_identity().interface_id())
            else {
                return Err(RegistryCompilationError::BindingUnknownInterface(
                    binding.binding_id().clone(),
                ));
            };
            if binding.interface_identity().version() != definition.version() {
                return Err(RegistryCompilationError::BindingVersionMismatch(
                    binding.binding_id().clone(),
                ));
            }
            if binding.contracts() != definition.contracts() {
                return Err(RegistryCompilationError::BindingContractMismatch(
                    binding.binding_id().clone(),
                ));
            }
            let effective_handler = effective_handlers
                .get(definition.interface_id())
                .expect("effective handler was compiled above")
                .clone();
            let authentication = self
                .authentication_activations
                .get(&(
                    definition.interface_id().clone(),
                    adapter_plan.authentication().clone(),
                ))
                .expect("authentication activation was validated above")
                .clone();
            let extension_plan = CompiledInterfaceExtensionPlan::compile(
                self.extensions
                    .get(definition.interface_id())
                    .cloned()
                    .unwrap_or_default(),
            )?;
            let binding_fingerprint = binding_fingerprint(binding);
            let fingerprint = plan_fingerprint(
                &self.graph_fingerprint,
                definition,
                &binding_fingerprint,
                adapter_plan,
                &authentication,
                extension_plan.fingerprint(),
                &effective_handler,
            );
            plans.insert(
                binding.binding_id().clone(),
                CompiledInvocationPlan {
                    definition: definition.clone(),
                    binding: binding.clone(),
                    binding_fingerprint,
                    adapter_plan: adapter_plan.clone(),
                    authentication,
                    extension_plan,
                    executable_extensions: self
                        .hook_bindings
                        .get(definition.interface_id())
                        .cloned(),
                    authorization_extensions: self
                        .authorization_bindings
                        .get(definition.interface_id())
                        .cloned(),
                    admission_extensions: self
                        .admission_bindings
                        .get(definition.interface_id())
                        .cloned(),
                    effective_handler,
                    managed_factory: self.managed_factory.clone(),
                    fingerprint,
                },
            );
        }
        let fingerprint = registry_fingerprint(
            &self.graph_fingerprint,
            &self.definitions,
            &self
                .protocol_bindings
                .iter()
                .map(|(id, (binding, _))| (id.clone(), binding.clone()))
                .collect(),
            &plans,
        );
        let schema_identity = managed_contracts
            .values()
            .map(|entry| {
                serde_json::json!({
                    "id": entry.contract.contract_id(), "version": entry.contract.version(),
                    "rust_type": entry.rust_type, "schema": entry.schema.as_deref(),
                })
            })
            .collect::<Vec<_>>();
        let mut digest = Sha256::new();
        digest.update(fingerprint.as_str().as_bytes());
        digest.update(
            serde_json::to_vec(&schema_identity).expect("static projection schemas serialize"),
        );
        let fingerprint = RegistryFingerprint::new(format!(
            "sha256:{:x}",
            base16ct::HexDisplay(&digest.finalize())
        ))
        .expect("schema fingerprint has a valid identity");
        Ok(Arc::new(CompiledInterfaceRegistry {
            managed_contracts,
            graph_fingerprint: self.graph_fingerprint,
            fingerprint,
            definitions: self.definitions,
            definition_contributions: self.compiled_definition_contributions,
            protocol_bindings: self
                .protocol_bindings
                .into_iter()
                .map(|(id, (binding, _))| (id, binding))
                .collect(),
            plans,
            routes: self.routes,
            handler_bindings: effective_handler_bindings,
        }))
    }
}

impl RegistryCompiler {
    pub fn with_managed_invocations(
        mut self,
        factory: Arc<dyn crate::ManagedInterfaceInvocationFactory>,
    ) -> Self {
        self.managed_factory = Some(factory);
        self
    }

    pub(super) fn install_managed_typed<I: InterfaceContract, O: InterfaceContract>(
        &mut self,
        id: &InterfaceId,
    ) -> Result<(), RegistryCompilationError> {
        use crate::hook::{managed_bridge_identity, ManagedLifecycleBridge};
        use crate::{
            InterfaceExtensionFact as Fact, InterfaceExtensionIsolation,
            InterfaceExtensionPermission as Permission, InterfaceExtensionTier,
        };
        let already_bound = self
            .extensions
            .get(id)
            .into_iter()
            .flatten()
            .filter(|(_, registration)| {
                registration.plugin() == &managed_bridge_identity(registration.point())
            })
            .count();
        if already_bound == 6 {
            return Ok(());
        }
        if already_bound != 0 {
            return Err(RegistryCompilationError::ManagedProjectionMismatch(
                id.clone(),
            ));
        }
        let bridge = Arc::new(ManagedLifecycleBridge);
        let graph = self.graph_fingerprint.clone();
        let hooks = self
            .hook_bindings
            .get(id)
            .map(|plan| {
                plan.as_any()
                    .downcast_ref::<TypedInterfaceHookPlan<I, O>>()
                    .cloned()
                    .ok_or_else(|| RegistryCompilationError::HookContractMismatch(id.clone()))
            })
            .transpose()?
            .unwrap_or_else(|| TypedInterfaceHookPlan::new(graph.clone()));
        let auth = self
            .authorization_bindings
            .get(id)
            .map(|plan| {
                plan.as_any()
                    .downcast_ref::<TypedInterfaceAuthorizationPlan<I, O>>()
                    .cloned()
                    .ok_or_else(|| RegistryCompilationError::HookContractMismatch(id.clone()))
            })
            .transpose()?
            .unwrap_or_else(|| TypedInterfaceAuthorizationPlan::new(graph.clone()));
        let admission = self
            .admission_bindings
            .get(id)
            .map(|plan| {
                plan.as_any()
                    .downcast_ref::<TypedInterfaceAdmissionPlan<I, O>>()
                    .cloned()
                    .ok_or_else(|| RegistryCompilationError::HookContractMismatch(id.clone()))
            })
            .transpose()?
            .unwrap_or_else(|| TypedInterfaceAdmissionPlan::new(graph));
        let scope = self.definitions[id].scope();
        for (point, permission, fact) in [
            (
                InterfaceExtensionPoint::Authorization,
                Permission::Authorize,
                Fact::PrincipalSummary,
            ),
            (
                InterfaceExtensionPoint::Admission,
                Permission::Admit,
                Fact::AuthorizationDecision,
            ),
            (
                InterfaceExtensionPoint::Before,
                Permission::ObserveInput,
                Fact::TypedInput,
            ),
            (
                InterfaceExtensionPoint::After,
                Permission::ObserveOutput,
                Fact::TypedOutput,
            ),
            (
                InterfaceExtensionPoint::Failure,
                Permission::ObserveFailure,
                Fact::FailureClassification,
            ),
            (
                InterfaceExtensionPoint::Completion,
                Permission::ObserveCompletion,
                Fact::Terminal,
            ),
        ] {
            self.register_extension(
                id,
                u32::MAX,
                InterfaceExtensionRegistration::new(
                    managed_bridge_identity(point),
                    InterfaceExtensionTier::HostExtension,
                    point,
                    permission,
                    scope,
                    InterfaceExtensionIsolation::TrustedInProcess,
                    vec![fact],
                )?,
            )?;
        }
        self.hook_bindings.insert(
            id.clone(),
            Arc::new(
                hooks
                    .bind_before(
                        managed_bridge_identity(InterfaceExtensionPoint::Before),
                        bridge.clone(),
                    )
                    .bind_after(
                        managed_bridge_identity(InterfaceExtensionPoint::After),
                        bridge.clone(),
                    )
                    .bind_failure(
                        managed_bridge_identity(InterfaceExtensionPoint::Failure),
                        bridge.clone(),
                    )
                    .bind_completion(
                        managed_bridge_identity(InterfaceExtensionPoint::Completion),
                        bridge.clone(),
                    ),
            ),
        );
        self.authorization_bindings.insert(
            id.clone(),
            Arc::new(auth.bind(
                managed_bridge_identity(InterfaceExtensionPoint::Authorization),
                bridge.clone(),
            )),
        );
        self.admission_bindings.insert(
            id.clone(),
            Arc::new(admission.bind(
                managed_bridge_identity(InterfaceExtensionPoint::Admission),
                bridge,
            )),
        );
        Ok(())
    }
}

fn intern_schema(
    schemas: &mut BTreeMap<Vec<u8>, Arc<serde_json::Value>>,
    schema: Arc<serde_json::Value>,
) -> Arc<serde_json::Value> {
    let bytes = serde_json::to_vec(schema.as_ref()).expect("static projection schemas serialize");
    schemas.entry(bytes).or_insert(schema).clone()
}

#[cfg(test)]
#[path = "../_tests/registry_schema_sharing.rs"]
mod schema_sharing_tests;
