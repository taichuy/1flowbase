use std::{collections::BTreeSet, sync::Arc};

use interface_runtime::{
    AuthorizationOperation, CompiledInterfaceRegistry, GraphFingerprint, InterfaceId,
    InterfaceOwner, RegistryCompilationError, RegistryCompiler,
};

type CompileInterfaceRegistry =
    fn(
        std::sync::Weak<crate::app_state::ApiState>,
    ) -> Result<Arc<CompiledInterfaceRegistry>, RegistryCompilationError>;

#[derive(Clone, Copy)]
pub(crate) struct InterfaceRegistryContribution {
    contribution_id: &'static str,
    authorization_operations: &'static [&'static str],
    owners: &'static [&'static str],
    compile: CompileInterfaceRegistry,
}

impl InterfaceRegistryContribution {
    pub(crate) const fn new(
        contribution_id: &'static str,
        authorization_operations: &'static [&'static str],
        owners: &'static [&'static str],
        compile: CompileInterfaceRegistry,
    ) -> Self {
        Self {
            contribution_id,
            authorization_operations,
            owners,
            compile,
        }
    }
}

struct PublishedInterfaceContribution {
    registry: Arc<CompiledInterfaceRegistry>,
    interface_id: InterfaceId,
    authorization_operation: AuthorizationOperation,
    owner: InterfaceOwner,
}

pub(crate) struct InterfaceContributionCollector {
    graph_fingerprint: GraphFingerprint,
    published: Vec<PublishedInterfaceContribution>,
    contributions: Vec<InterfaceRegistryContribution>,
    contribution_ids: BTreeSet<&'static str>,
}

impl InterfaceContributionCollector {
    pub(crate) fn new(graph_fingerprint: GraphFingerprint) -> Self {
        Self {
            graph_fingerprint,
            published: Vec::new(),
            contributions: Vec::new(),
            contribution_ids: BTreeSet::new(),
        }
    }

    pub(crate) fn absorb_published_interface(
        &mut self,
        registry: Arc<CompiledInterfaceRegistry>,
        interface_id: InterfaceId,
        authorization_operation: AuthorizationOperation,
        owner: InterfaceOwner,
    ) {
        self.published.push(PublishedInterfaceContribution {
            registry,
            interface_id,
            authorization_operation,
            owner,
        });
    }

    pub(crate) fn add(
        &mut self,
        contribution: InterfaceRegistryContribution,
    ) -> anyhow::Result<()> {
        if !self.contribution_ids.insert(contribution.contribution_id) {
            anyhow::bail!(
                "duplicate interface registry contribution `{}`",
                contribution.contribution_id
            );
        }
        self.contributions.push(contribution);
        Ok(())
    }

    pub(crate) fn compile(
        self,
        state: std::sync::Weak<crate::app_state::ApiState>,
    ) -> anyhow::Result<Arc<CompiledInterfaceRegistry>> {
        let mut operations = BTreeSet::new();
        let mut owners = BTreeSet::new();
        for published in &self.published {
            operations.insert(published.authorization_operation.clone());
            owners.insert(published.owner.clone());
        }
        for contribution in &self.contributions {
            for operation in contribution.authorization_operations {
                operations.insert(AuthorizationOperation::new(operation)?);
            }
            for owner in contribution.owners {
                owners.insert(InterfaceOwner::new(owner)?);
            }
        }

        let mut compiler = RegistryCompiler::new(self.graph_fingerprint, operations, owners);
        for published in self.published {
            compiler.absorb_interface(published.registry.as_ref(), &published.interface_id)?;
        }
        for contribution in self.contributions {
            compiler.absorb_snapshot((contribution.compile)(state.clone())?.as_ref())?;
        }
        Ok(compiler.compile()?)
    }
}

pub(crate) fn production_interface_contributions() -> [InterfaceRegistryContribution; 7] {
    [
        InterfaceRegistryContribution::new(
            "api-server.public-login-instances",
            &["public.auth.login-instances.read"],
            &["api-server.public-auth"],
            crate::routes::auth::compile_public_login_instances_registry,
        ),
        InterfaceRegistryContribution::new(
            "api-server.public-sign-in",
            &["public.auth.sign-in"],
            &["api-server.public-auth"],
            crate::routes::sign_in_interface::compile_registry,
        ),
        InterfaceRegistryContribution::new(
            "api-server.public-auth-residual",
            &["public.auth.providers.read", "public.auth.sign-up"],
            &["api-server.public-auth"],
            crate::routes::auth::compile_public_residual_registry,
        ),
        InterfaceRegistryContribution::new(
            "api-server.native-runs",
            &["application.native.runs.create"],
            &["api-server.application-public-api"],
            compile_native_runs,
        ),
        InterfaceRegistryContribution::new(
            "api-server.compatibility",
            &["application.native.runs.create"],
            &["api-server.application-public-api"],
            crate::routes::application_public_api::compatibility_interface::compile_registry,
        ),
        InterfaceRegistryContribution::new(
            "api-server.mcp",
            &["mcp.tools.invoke"],
            &["api-server.mcp-protocol"],
            crate::routes::mcp_protocol::compile_mcp_interface_registry,
        ),
        InterfaceRegistryContribution::new(
            "api-server.workflow-extension",
            &["workflow-extension.invoke"],
            &["api-server.workflow-extension"],
            crate::routes::application_public_api::ex::compile_workflow_extension_registry,
        ),
    ]
}

fn compile_native_runs(
    state: std::sync::Weak<crate::app_state::ApiState>,
) -> Result<Arc<CompiledInterfaceRegistry>, RegistryCompilationError> {
    let state = state
        .upgrade()
        .expect("native contribution is assembled while API state is alive");
    crate::routes::application_public_api::native::compile_native_interface_registry(state)
}
