use std::sync::Arc;

use control_plane::auth::settings::{
    AuthCenterSettingsService, CopyAuthCenterAuthenticatorCommand,
    CreateAuthCenterAuthenticatorCommand, UpdateAuthCenterAuthenticatorConfigCommand,
    UpdateAuthCenterAuthenticatorPublicUiBlockCommand,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::auth_center;
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError, ConsoleLocaleHints,
    },
};

pub(crate) enum AuthCenterInput {
    Overview {
        locale: ConsoleLocaleHints,
    },
    Create {
        locale: ConsoleLocaleHints,
        body: auth_center::CreateAuthCenterAuthenticatorBody,
    },
    Copy {
        locale: ConsoleLocaleHints,
        id: Uuid,
        body: auth_center::CopyAuthCenterAuthenticatorBody,
    },
    Delete {
        id: Uuid,
    },
    Reorder {
        locale: ConsoleLocaleHints,
        body: auth_center::ReorderAuthCenterAuthenticatorsBody,
    },
    Enable {
        locale: ConsoleLocaleHints,
        id: Uuid,
    },
    UpdateConfig {
        locale: ConsoleLocaleHints,
        id: Uuid,
        body: auth_center::UpdateAuthCenterAuthenticatorConfigBody,
    },
    UpdatePublicUiBlock {
        locale: ConsoleLocaleHints,
        id: Uuid,
        body: auth_center::UpdateAuthCenterAuthenticatorPublicUiBlockBody,
    },
}

impl InterfaceContract for AuthCenterInput {
    const CONTRACT_ID: &'static str = "console-auth-center-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed authentication output is projected immediately into the console response"
)]
pub(crate) enum AuthCenterOutput {
    Overview(auth_center::AuthCenterOverviewResponse),
    Authenticator(auth_center::AuthCenterAuthenticatorResponse),
    NoContent,
}

impl InterfaceContract for AuthCenterOutput {
    const CONTRACT_ID: &'static str = "console-auth-center-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct AuthCenterAdapter {
    store: MainDurableStore,
    registry: Arc<control_plane::auth::AuthenticatorRegistry>,
    bootstrap_workspace_id: Uuid,
}

pub(crate) fn auth_center_port(
    store: MainDurableStore,
    registry: Arc<control_plane::auth::AuthenticatorRegistry>,
    bootstrap_workspace_id: Uuid,
) -> Arc<dyn ConsoleInterfacePort<AuthCenterInput, AuthCenterOutput>> {
    Arc::new(AuthCenterAdapter {
        store,
        registry,
        bootstrap_workspace_id,
    })
}

impl AuthCenterAdapter {
    async fn locale(
        &self,
        principal: &UserPrincipal,
        hints: &ConsoleLocaleHints,
    ) -> Result<domain::CatalogLocale, ApiError> {
        let preferred = self
            .store
            .find_user_by_id(principal.actor().user_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
            .preferred_locale;
        Ok(hints.resolve(preferred))
    }

    async fn authenticator_response(
        &self,
        principal: &UserPrincipal,
        hints: ConsoleLocaleHints,
        authenticator: domain::AuthenticatorRecord,
    ) -> Result<auth_center::AuthCenterAuthenticatorResponse, ApiError> {
        let locale = self.locale(principal, &hints).await?;
        let mut response = auth_center::to_auth_center_authenticator_response(
            authenticator,
            self.registry.as_ref(),
        );
        auth_center::localize_authenticator_response_with(
            &self.store,
            self.bootstrap_workspace_id,
            &locale,
            &mut response,
        )
        .await?;
        Ok(response)
    }

    async fn overview_response(
        &self,
        principal: &UserPrincipal,
        hints: ConsoleLocaleHints,
        overview: control_plane::auth::settings::AuthCenterSettingsOverview,
    ) -> Result<auth_center::AuthCenterOverviewResponse, ApiError> {
        let locale = self.locale(principal, &hints).await?;
        let mut response =
            auth_center::auth_center_overview_response(overview, self.registry.as_ref());
        auth_center::localize_overview_response_with(
            &self.store,
            self.bootstrap_workspace_id,
            &locale,
            &mut response,
        )
        .await?;
        Ok(response)
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: AuthCenterInput,
    ) -> Result<AuthCenterOutput, ApiError> {
        let actor = principal.actor();
        let service = AuthCenterSettingsService::with_registry(
            self.store.clone(),
            Arc::clone(&self.registry),
        );
        match input {
            AuthCenterInput::Overview { locale } => {
                let overview = service.overview(actor).await?;
                Ok(AuthCenterOutput::Overview(
                    self.overview_response(principal, locale, overview).await?,
                ))
            }
            AuthCenterInput::Create { locale, body } => {
                let record = service
                    .create_authenticator(
                        actor,
                        CreateAuthCenterAuthenticatorCommand {
                            auth_type: body.auth_type,
                            title: body.title,
                            description: body.description,
                            enabled: body.enabled,
                            sort_order: body.sort_order,
                        },
                    )
                    .await?;
                Ok(AuthCenterOutput::Authenticator(
                    self.authenticator_response(principal, locale, record)
                        .await?,
                ))
            }
            AuthCenterInput::Copy { locale, id, body } => {
                let record = service
                    .copy_authenticator(
                        actor,
                        CopyAuthCenterAuthenticatorCommand {
                            source_id: id,
                            title: body.title,
                            sort_order: body.sort_order,
                        },
                    )
                    .await?;
                Ok(AuthCenterOutput::Authenticator(
                    self.authenticator_response(principal, locale, record)
                        .await?,
                ))
            }
            AuthCenterInput::Delete { id } => {
                service.delete_authenticator(actor, id).await?;
                Ok(AuthCenterOutput::NoContent)
            }
            AuthCenterInput::Reorder { locale, body } => {
                let overview = service.reorder_authenticators(actor, &body.ids).await?;
                Ok(AuthCenterOutput::Overview(
                    self.overview_response(principal, locale, overview).await?,
                ))
            }
            AuthCenterInput::Enable { locale, id } => {
                let record = service.enable_authenticator(actor, id).await?;
                Ok(AuthCenterOutput::Authenticator(
                    self.authenticator_response(principal, locale, record)
                        .await?,
                ))
            }
            AuthCenterInput::UpdateConfig { locale, id, body } => {
                let record = service
                    .update_authenticator_config(
                        actor,
                        UpdateAuthCenterAuthenticatorConfigCommand {
                            authenticator_id: id,
                            title: body.title,
                            enabled: body.enabled,
                            description: body.description,
                            self_registration_enabled: body.self_registration_enabled,
                            extension_config: body.extension_config,
                        },
                    )
                    .await?;
                Ok(AuthCenterOutput::Authenticator(
                    self.authenticator_response(principal, locale, record)
                        .await?,
                ))
            }
            AuthCenterInput::UpdatePublicUiBlock { locale, id, body } => {
                let record = service
                    .update_authenticator_public_ui_block(
                        actor,
                        UpdateAuthCenterAuthenticatorPublicUiBlockCommand {
                            authenticator_id: id,
                            public_ui_block: body.public_ui_block,
                        },
                    )
                    .await?;
                Ok(AuthCenterOutput::Authenticator(
                    self.authenticator_response(principal, locale, record)
                        .await?,
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<AuthCenterInput, AuthCenterOutput> for AuthCenterAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: AuthCenterInput,
    ) -> ConsoleInterfaceFuture<'a, AuthCenterOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.overview.view",
        binding_id: "http.console.auth-center.overview.view.v1",
        method: "GET",
        path: "/api/console/settings/auth-center/overview",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.authenticators.create",
        binding_id: "http.console.auth-center.authenticators.create.v1",
        method: "POST",
        path: "/api/console/settings/auth-center/authenticators",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.authenticators.order",
        binding_id: "http.console.auth-center.authenticators.order.v1",
        method: "PUT",
        path: "/api/console/settings/auth-center/authenticators/order",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.authenticators.enable",
        binding_id: "http.console.auth-center.authenticators.enable.v1",
        method: "POST",
        path: "/api/console/settings/auth-center/authenticators/:id/actions/enable",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.authenticators.copy",
        binding_id: "http.console.auth-center.authenticators.copy.v1",
        method: "POST",
        path: "/api/console/settings/auth-center/authenticators/:id/copy",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.authenticators.update.config",
        binding_id: "http.console.auth-center.authenticators.update.config.v1",
        method: "PUT",
        path: "/api/console/settings/auth-center/authenticators/:id/config",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.authenticators.update.public-ui-block",
        binding_id: "http.console.auth-center.authenticators.update.public-ui-block.v1",
        method: "PUT",
        path: "/api/console/settings/auth-center/authenticators/:id/public-ui-block",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "auth_center.authenticators.delete",
        binding_id: "http.console.auth-center.authenticators.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/auth-center/authenticators/:id",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<AuthCenterInput, AuthCenterOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-auth-center",
        "graph:console-auth-center-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableAuthCenterPort;

#[cfg(test)]
impl ConsoleInterfacePort<AuthCenterInput, AuthCenterOutput> for UnavailableAuthCenterPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: AuthCenterInput,
    ) -> ConsoleInterfaceFuture<'a, AuthCenterOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("auth center fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use interface_runtime::BindingId;

    use super::*;

    #[test]
    fn f08c_registry_freezes_auth_center_bindings() {
        let registry = compile_registry(Arc::new(UnavailableAuthCenterPort)).unwrap();
        for declaration in DECLARATIONS {
            assert!(registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .is_some());
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
