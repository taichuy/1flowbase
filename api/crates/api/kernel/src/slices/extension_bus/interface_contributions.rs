use std::{collections::BTreeSet, sync::Arc};

use interface_runtime::{
    AuthorizationOperation, CompiledInterfaceRegistry, GraphFingerprint, InterfaceOwner,
    RegistryCompiler,
};

#[derive(Clone)]
pub struct InterfaceRegistryContribution {
    contribution_id: String,
    authorization_operations: Vec<String>,
    owners: Vec<String>,
    registry: Arc<CompiledInterfaceRegistry>,
}

impl InterfaceRegistryContribution {
    pub fn contribution_id(&self) -> &str {
        &self.contribution_id
    }
    pub fn registry(&self) -> &Arc<CompiledInterfaceRegistry> {
        &self.registry
    }

    pub fn new(
        contribution_id: &'static str,
        authorization_operations: &'static [&'static str],
        owners: &'static [&'static str],
        registry: Arc<CompiledInterfaceRegistry>,
    ) -> Self {
        Self {
            contribution_id: contribution_id.into(),
            authorization_operations: authorization_operations
                .iter()
                .map(|s| s.to_string())
                .collect(),
            owners: owners.iter().map(|s| s.to_string()).collect(),
            registry,
        }
    }

    pub fn managed(
        owner: String,
        operations: Vec<String>,
        registry: Arc<CompiledInterfaceRegistry>,
    ) -> Self {
        Self {
            contribution_id: format!("managed-service.{owner}"),
            authorization_operations: operations,
            owners: vec![owner],
            registry,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_test_contribution_id(mut self, contribution_id: &'static str) -> Self {
        self.contribution_id = contribution_id.into();
        self
    }
}

pub struct InterfaceContributionCollector {
    api_docs: Option<Arc<crate::openapi_docs::ApiDocsRegistry>>,
    document_observer: Option<Arc<dyn interface_runtime::DocumentOperationObserver>>,
    managed_factory: Option<Arc<dyn interface_runtime::ManagedInterfaceInvocationFactory>>,
    event_observer: Option<Arc<dyn interface_runtime::InterfaceEventObserver>>,
    graph_fingerprint: GraphFingerprint,
    contributions: Vec<InterfaceRegistryContribution>,
    contribution_ids: BTreeSet<String>,
}

impl InterfaceContributionCollector {
    pub fn contributions(&self) -> &[InterfaceRegistryContribution] {
        &self.contributions
    }

    pub fn new(graph_fingerprint: GraphFingerprint) -> Self {
        Self {
            api_docs: None,
            document_observer: None,
            managed_factory: None,
            event_observer: None,
            graph_fingerprint,
            contributions: Vec::new(),
            contribution_ids: BTreeSet::new(),
        }
    }

    pub fn with_api_event_observation(
        mut self,
        docs: Arc<crate::openapi_docs::ApiDocsRegistry>,
        observer: Arc<dyn interface_runtime::DocumentOperationObserver>,
    ) -> Self {
        self.api_docs = Some(docs);
        self.document_observer = Some(observer);
        self
    }

    pub fn with_event_observer(
        mut self,
        observer: Arc<dyn interface_runtime::InterfaceEventObserver>,
    ) -> Self {
        self.event_observer = Some(observer);
        self
    }

    pub fn add(&mut self, contribution: InterfaceRegistryContribution) -> anyhow::Result<()> {
        if !self
            .contribution_ids
            .insert(contribution.contribution_id.clone())
        {
            anyhow::bail!(
                "duplicate interface registry contribution `{}`",
                contribution.contribution_id
            );
        }
        self.contributions.push(contribution);
        Ok(())
    }

    pub fn compile(self) -> anyhow::Result<Arc<CompiledInterfaceRegistry>> {
        let mut operations = BTreeSet::new();
        let mut owners = BTreeSet::new();
        for contribution in &self.contributions {
            for operation in &contribution.authorization_operations {
                operations.insert(AuthorizationOperation::new(operation)?);
            }
            for owner in &contribution.owners {
                owners.insert(InterfaceOwner::new(owner)?);
            }
        }

        let mut compiler = RegistryCompiler::new(self.graph_fingerprint, operations, owners);
        if let Some(factory) = self.managed_factory {
            compiler = compiler.with_managed_invocations(factory);
        }
        if let Some(observer) = self.event_observer {
            compiler = compiler.with_event_observer(observer);
        }
        if let Some(observer) = self.document_observer {
            compiler = compiler.with_document_operation_observer(observer);
        }
        let snapshots = self
            .contributions
            .iter()
            .map(|contribution| contribution.registry.clone())
            .collect::<Vec<_>>();
        for contribution in self.contributions {
            compiler.absorb_snapshot(contribution.registry.as_ref())?;
        }
        if let Some(docs) = self.api_docs {
            crate::openapi_interface::contract::refine_static_api_operations(
                &mut compiler,
                &docs,
                &snapshots,
            )?;
        }
        Ok(compiler.compile()?)
    }
}

impl InterfaceContributionCollector {
    pub fn with_managed_invocations(
        mut self,
        factory: Arc<dyn interface_runtime::ManagedInterfaceInvocationFactory>,
    ) -> Self {
        self.managed_factory = Some(factory);
        self
    }
}
