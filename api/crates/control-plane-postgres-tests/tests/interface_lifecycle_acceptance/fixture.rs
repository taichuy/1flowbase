use control_plane::{
    model_definition::{CreateModelDefinitionCommand, ModelDefinitionService},
    ports::AuthRepository,
};
use control_plane_contracts::ports::{
    LifecyclePublicationCatalog, LifecyclePublicationPlan, LifecycleSubscriberTarget,
    ModelDefinitionCommittedFact,
};
use extension_contracts::LifecycleContract;
use interface_runtime::*;
use std::sync::{Arc, Mutex};
use storage_durable_postgres::PgControlPlaneStore;
use tokio::sync::Notify;

pub const GRAPH: &str = "graph:root-1998-transaction";
pub struct Fixture {
    pub store: PgControlPlaneStore,
    pub actor: domain::ActorContext,
}
impl Fixture {
    pub async fn new() -> Self {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
        let schema = postgres_test_support::PostgresTestSchema::create(&url)
            .await
            .unwrap();
        let pool = schema.connect().await.unwrap();
        storage_durable_postgres::run_migrations(&pool)
            .await
            .unwrap();
        let store = PgControlPlaneStore::new(pool).with_lifecycle_publication_catalog(
            LifecyclePublicationCatalog::new([(
                (
                    ModelDefinitionCommittedFact::CONTRACT_ID.into(),
                    ModelDefinitionCommittedFact::CONTRACT_VERSION.into(),
                ),
                LifecyclePublicationPlan {
                    graph_fingerprint: GRAPH.into(),
                    subscribers: vec![LifecycleSubscriberTarget {
                        subscriber_id: "fixture.subscriber".into(),
                        handler_id: "fixture.delivery".into(),
                        handler_version: "1".into(),
                    }],
                },
            )])
            .unwrap(),
        );
        let tenant = store.upsert_root_tenant().await.unwrap();
        let workspace = store
            .upsert_workspace(tenant.id, "Root 1998 transaction fixture")
            .await
            .unwrap();
        control_plane_test_support::upsert_permission_catalog(&store)
            .await
            .unwrap();
        control_plane_test_support::upsert_builtin_roles(&store, workspace.id)
            .await
            .unwrap();
        store
            .upsert_login_entry(&domain::LoginEntryRecord {
                id: domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID,
                connection_id: domain::PASSWORD_LOCAL_CONNECTION_ID,
                auth_type: "password-local".into(),
                title: "Password".into(),
                enabled: true,
                is_builtin: true,
                sort_order: 0,
                public_ui_block: String::new(),
                options: serde_json::json!({}),
            })
            .await
            .unwrap();
        let user = store
            .upsert_root_user(
                workspace.id,
                "root",
                "root@example.com",
                "$argon2id$v=19$m=19456,t=2,p=1$test$test",
                "Root",
                "Root",
            )
            .await
            .unwrap();
        let actor = AuthRepository::load_actor_context_for_user(&store, user.id)
            .await
            .unwrap();
        Self { store, actor }
    }
    pub fn envelope(&self, code: &str) -> InvocationEnvelope<CreateInput> {
        InvocationEnvelope::new(
            InvocationLineage::root(InvocationId::now_v7()),
            BindingId::new("http.fixture.root-1998.create.v1").unwrap(),
            InterfaceProtocol::Http,
            AuthenticationAdapterReference::new("fixture.authn").unwrap(),
            AuthenticationActivationIdentity::new("fixture.authn.v1").unwrap(),
            self.actor.clone(),
            None,
            CreateInput(CreateModelDefinitionCommand {
                actor_user_id: self.actor.user_id,
                scope_kind: domain::DataModelScopeKind::Workspace,
                data_source_instance_id: None,
                external_resource_key: None,
                external_table_id: None,
                external_capabilities: None,
                template_provider: "core".into(),
                template_code: "general".into(),
                template_version: "v1".into(),
                code: code.into(),
                title: "Root 1998 model".into(),
                description: None,
                status: Some(domain::DataModelStatus::Published),
            }),
        )
    }
}
pub struct CreateInput(pub CreateModelDefinitionCommand);
impl InterfaceContract for CreateInput {
    const CONTRACT_ID: &'static str = "fixture-create-input";
    const CONTRACT_VERSION: &'static str = "1";
}
#[derive(Debug)]
pub struct CreateOutput(pub domain::ModelDefinitionRecord);
impl InterfaceContract for CreateOutput {
    const CONTRACT_ID: &'static str = "fixture-create-output";
    const CONTRACT_VERSION: &'static str = "1";
}
#[derive(Debug)]
pub struct CreateError(pub String);
impl InterfaceContract for CreateError {
    const CONTRACT_ID: &'static str = "fixture-create-error";
    const CONTRACT_VERSION: &'static str = "1";
}

pub struct CreateHandler {
    pub store: PgControlPlaneStore,
    pub hold_after_commit: Option<Arc<Notify>>,
}
impl InterfaceHandler<CreateInput, CreateOutput, CreateError> for CreateHandler {
    fn invoke(
        &self,
        _: InterfaceHandlerContext,
        input: CreateInput,
    ) -> InterfaceHandlerFuture<CreateOutput, CreateError> {
        let store = self.store.clone();
        let hold = self.hold_after_commit.clone();
        Box::pin(async move {
            let model = ModelDefinitionService::new(store)
                .create_model(input.0)
                .await
                .map_err(|error| {
                    InterfaceTargetFailure::new(
                        "model_create_failed",
                        CreateError(error.to_string()),
                    )
                })?;
            if let Some(committed) = hold {
                committed.notify_one();
                std::future::pending::<()>().await;
            }
            Ok(CreateOutput(model))
        })
    }
}
pub struct CoreAuthorization;
impl InterfaceAuthorizationPort for CoreAuthorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("fixture.authz").unwrap()
    }
    fn authorize(
        &self,
        request: InterfaceAuthorizationRequest,
    ) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async move {
            if request.principal().actor().is_root {
                Ok(())
            } else {
                Err(InterfaceAuthorizationError::classified("not_root"))
            }
        })
    }
}
pub struct Completion(pub Arc<Mutex<Vec<InterfaceInvocationTerminal>>>);
impl InterfaceCompletionHook for Completion {
    fn completed(
        &self,
        _: InterfaceHookContext,
        terminal: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_> {
        Box::pin(async move {
            self.0.lock().unwrap().push(terminal);
        })
    }
}
pub fn registry(
    handler: CreateHandler,
    completed: Arc<Mutex<Vec<InterfaceInvocationTerminal>>>,
) -> Arc<CompiledInterfaceRegistry> {
    let id = InterfaceId::new("fixture.model.create").unwrap();
    let owner = InterfaceOwner::new("fixture").unwrap();
    let operation = AuthorizationOperation::new("model_definitions.create").unwrap();
    let graph = GraphFingerprint::new(GRAPH).unwrap();
    let mut compiler = RegistryCompiler::new(graph.clone(), [operation.clone()], [owner.clone()]);
    let definition = InterfaceDefinition::new(
        InterfaceIdentity::new(id.clone(), InterfaceVersion::new("1").unwrap()),
        InterfaceContracts::unary(
            ContractIdentity::new(CreateInput::CONTRACT_ID, "1").unwrap(),
            ContractIdentity::new(CreateOutput::CONTRACT_ID, "1").unwrap(),
            ContractIdentity::new(CreateError::CONTRACT_ID, "1").unwrap(),
        ),
        InterfaceAccess::new(
            PrincipalProfile::User,
            InterfaceAuthenticationPolicy::Authenticated,
            operation,
            InterfaceScope::Workspace,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            HandlerReference::new("fixture.create.handler").unwrap(),
            TargetReference::new("control-plane.model_definition.create").unwrap(),
        ),
        InterfaceAuditPolicy::Mutating,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner,
    );
    compiler.register_definition(definition.clone()).unwrap();
    let authn = PluginIdentity::new("fixture.authn").unwrap();
    compiler
        .register_authentication_adapter(
            &id,
            0,
            InterfaceExtensionRegistration::new(
                authn.clone(),
                InterfaceExtensionTier::BuiltIn,
                InterfaceExtensionPoint::AuthenticationAdapter,
                InterfaceExtensionPermission::Authenticate,
                InterfaceScope::Workspace,
                InterfaceExtensionIsolation::TrustedInProcess,
                [],
            )
            .unwrap(),
            ActivatedAuthenticationAdapter::new(
                authn,
                InterfaceExtensionTier::BuiltIn,
                AuthenticationAdapterReference::new("fixture.authn").unwrap(),
                AuthenticationActivationIdentity::new("fixture.authn.v1").unwrap(),
                PrincipalProfile::User,
            ),
        )
        .unwrap();
    compiler
        .register_binding(
            ProtocolBinding::new(
                BindingId::new("http.fixture.root-1998.create.v1").unwrap(),
                definition.identity().clone(),
                definition.contracts().clone(),
                ProtocolProjection::http(RouteIdentity::new("POST", "/fixture/model").unwrap()),
            ),
            InvocationAdapterPlan::new(
                AuthenticationAdapterReference::new("fixture.authn").unwrap(),
                AuthorizationAdapterReference::new("fixture.authz").unwrap(),
                None,
            ),
        )
        .unwrap();
    compiler
        .bind_handler::<CreateInput, CreateOutput, CreateError, UserPrincipal>(
            &id,
            definition.handler_reference().clone(),
            Arc::new(handler),
        )
        .unwrap();
    let observer = PluginIdentity::new("fixture.completion").unwrap();
    compiler
        .register_extension(
            &id,
            1,
            InterfaceExtensionRegistration::new(
                observer.clone(),
                InterfaceExtensionTier::BuiltIn,
                InterfaceExtensionPoint::Completion,
                InterfaceExtensionPermission::ObserveCompletion,
                InterfaceScope::Workspace,
                InterfaceExtensionIsolation::TrustedInProcess,
                [InterfaceExtensionFact::Terminal],
            )
            .unwrap(),
        )
        .unwrap();
    compiler
        .bind_hook_plan(
            &id,
            Arc::new(
                TypedInterfaceHookPlan::<CreateInput, CreateOutput>::new(graph)
                    .bind_completion(observer, Arc::new(Completion(completed))),
            ),
        )
        .unwrap();
    compiler.compile().unwrap()
}
