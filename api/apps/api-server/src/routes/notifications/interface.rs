use crate::{
    app_state::ApiNotificationService,
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};
use control_plane::notifications::{
    CreateNotificationInstance, NotificationInstanceListInput, NotificationSenderPort,
    PatchNotificationInstance,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

pub(crate) enum NotificationInput {
    Types,
    List(NotificationInstanceListInput),
    Get(Uuid),
    Create(CreateNotificationInstance),
    Patch(Uuid, PatchNotificationInstance),
    Delete(Uuid),
    Send(Uuid, Value),
    Authorize(
        Uuid,
        extension_contracts::NotificationAuthorizationOperation,
    ),
    Messages(control_plane::ports::NotificationMessageListInput),
    UnreadCount,
    Read(Uuid),
    ReadAll,
    Logs(control_plane::ports::NotificationLogListInput),
    Log(Uuid),
    Channels(control_plane::ports::NotificationChannelListInput),
    ReadChannel(Uuid),
    MarkUnread(Uuid),
}
impl InterfaceContract for NotificationInput {
    const CONTRACT_ID: &'static str = "console-notifications-input";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> {
        Some(crate::extension_bus::managed_projection::object_schema([(
            "operation",
            crate::extension_bus::managed_projection::text_schema(),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(
            serde_json::json!({"operation":match self {Self::Types=>"types",Self::List(_)=>"list",Self::Get(_)=>"get",Self::Create(_)=>"create",Self::Patch(..)=>"patch",Self::Delete(_)=>"delete",Self::Send(..)=>"send",Self::Authorize(..)=>"authorization",Self::Messages(_)=>"messages",Self::UnreadCount=>"unread_count",Self::Read(_)=>"read",Self::ReadAll=>"read_all",Self::Logs(_)=>"logs",Self::Log(_)=>"log",Self::Channels(_)=>"channels",Self::ReadChannel(_)=>"read_channel",Self::MarkUnread(_)=>"mark_unread"}}),
        )
    }
}
pub(crate) struct NotificationOutput(pub Value);
impl InterfaceContract for NotificationOutput {
    const CONTRACT_ID: &'static str = "console-notifications-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> {
        Some(crate::extension_bus::managed_projection::json_summary_schema())
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(crate::extension_bus::managed_projection::json_summary(
            &self.0,
        ))
    }
}
struct Adapter {
    service: Arc<ApiNotificationService>,
    communication: crate::routes::communications::interface::Dependencies,
}
impl Adapter {
    async fn inner(
        &self,
        principal: &UserPrincipal,
        input: NotificationInput,
    ) -> Result<NotificationOutput, ApiError> {
        let actor = principal.actor();
        let value = match input {
            NotificationInput::Types => serde_json::to_value(self.service.types(actor).await?)?,
            NotificationInput::List(input) => {
                serde_json::to_value(self.service.query_instances(actor, input).await?)?
            }
            NotificationInput::Get(id) => {
                serde_json::to_value(self.service.get_instance(actor, id).await?)?
            }
            NotificationInput::Create(input) => {
                let instance = self.service.create_instance(actor, input).await?;
                crate::routes::communications::provisioning::ensure_channel_mcp(
                    &self.communication,
                    actor,
                    instance.record.id,
                )
                .await?;
                serde_json::to_value(instance)?
            }
            NotificationInput::Patch(id, input) => {
                serde_json::to_value(self.service.patch_instance(actor, id, input).await?)?
            }
            NotificationInput::Delete(id) => {
                self.service.delete_instance(actor, id).await?;
                Value::Null
            }
            NotificationInput::Send(id, input) => serde_json::to_value(
                self.service
                    .send(actor, domain::NotificationSource::Api, id, input)
                    .await?,
            )?,
            NotificationInput::Authorize(id, operation) => serde_json::to_value(
                self.service
                    .authorize_instance(actor, id, operation)
                    .await?,
            )?,
            NotificationInput::Messages(input) => {
                serde_json::to_value(self.service.messages(actor, input).await?)?
            }
            NotificationInput::UnreadCount => {
                serde_json::json!({"unread_count":self.service.unread_count(actor).await?})
            }
            NotificationInput::Read(id) => {
                serde_json::to_value(self.service.read(actor, id).await?)?
            }
            NotificationInput::Logs(input) => {
                serde_json::to_value(self.service.logs(actor, input).await?)?
            }
            NotificationInput::Log(id) => serde_json::to_value(self.service.log(actor, id).await?)?,
            NotificationInput::Channels(input) => {
                serde_json::to_value(self.service.channels(actor, input).await?)?
            }
            NotificationInput::ReadChannel(id) => {
                serde_json::json!({"updated_count":self.service.read_channel(actor,id).await?})
            }
            NotificationInput::MarkUnread(id) => {
                serde_json::to_value(self.service.unread(actor, id).await?)?
            }
            NotificationInput::ReadAll => {
                serde_json::json!({"updated_count":self.service.read_all(actor).await?})
            }
        };
        Ok(NotificationOutput(value))
    }
}
impl ConsoleInterfacePort<NotificationInput, NotificationOutput> for Adapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: NotificationInput,
    ) -> ConsoleInterfaceFuture<'a, NotificationOutput> {
        Box::pin(async move {
            self.inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}
pub(crate) fn compile_registry(
    service: Arc<ApiNotificationService>,
    communication: crate::routes::communications::interface::Dependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-notifications",
        "graph:console-notifications-v1",
        DECLARATIONS,
        Arc::new(Adapter {
            service,
            communication,
        }),
    )
}
pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.channels.list",
        binding_id: "http.console.notifications.channels.list.v1",
        method: "GET",
        path: "/api/console/notifications/channels",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.channels.read_all",
        binding_id: "http.console.notifications.channels.read_all.v1",
        method: "POST",
        path: "/api/console/notifications/channels/:instance_id/read-all",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.messages.unread",
        binding_id: "http.console.notifications.messages.unread.v1",
        method: "POST",
        path: "/api/console/notifications/messages/:notification_id/unread",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.logs.list",
        binding_id: "http.console.notifications.logs.list.v1",
        method: "GET",
        path: "/api/console/communications/logs",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.logs.get",
        binding_id: "http.console.notifications.logs.get.v1",
        method: "GET",
        path: "/api/console/communications/logs/:log_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.types.list",
        binding_id: "http.console.notifications.types.list.v1",
        method: "GET",
        path: "/api/console/communications/types",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.instances.list",
        binding_id: "http.console.notifications.instances.list.v1",
        method: "GET",
        path: "/api/console/communications/instances",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.instances.get",
        binding_id: "http.console.notifications.instances.get.v1",
        method: "GET",
        path: "/api/console/communications/instances/:instance_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.instances.create",
        binding_id: "http.console.notifications.instances.create.v1",
        method: "POST",
        path: "/api/console/communications/instances",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.instances.update",
        binding_id: "http.console.notifications.instances.update.v1",
        method: "PATCH",
        path: "/api/console/communications/instances/:instance_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.instances.delete",
        binding_id: "http.console.notifications.instances.delete.v1",
        method: "DELETE",
        path: "/api/console/communications/instances/:instance_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.instances.authorization",
        binding_id: "http.console.notifications.instances.authorization.v1",
        method: "POST",
        path: "/api/console/communications/instances/:instance_id/authorization",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.instances.send",
        binding_id: "http.console.notifications.instances.send.v1",
        method: "POST",
        path: "/api/console/communications/instances/:instance_id/send",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.messages.list",
        binding_id: "http.console.notifications.messages.list.v1",
        method: "GET",
        path: "/api/console/notifications/messages",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.messages.unread_count",
        binding_id: "http.console.notifications.messages.unread_count.v1",
        method: "GET",
        path: "/api/console/notifications/messages/unread-count",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.messages.read",
        binding_id: "http.console.notifications.messages.read.v1",
        method: "POST",
        path: "/api/console/notifications/messages/:notification_id/read",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "notifications.messages.read_all",
        binding_id: "http.console.notifications.messages.read_all.v1",
        method: "POST",
        path: "/api/console/notifications/messages/read-all",
        mutating: true,
    },
];
