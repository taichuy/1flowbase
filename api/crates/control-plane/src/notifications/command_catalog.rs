//! Same effective command declarations feed settings, help and parsing.
use super::*;
use control_plane_contracts::ports::FrozenCommunicationCommandTarget;
use domain::{
    CommunicationCommandSetting, CommunicationCommandSourceKind, CommunicationCommandView,
};
use extension_contracts::{
    CommunicationCommandReply, CommunicationCommandReplyContentError,
    CommunicationMessageBlock as Block, CommunicationMessageContent as Content,
    CommunicationMessageInline as Inline,
};
use plugin_framework::communication_commands::{
    effective_communication_commands, parse_communication_command, CommunicationCommandCatalogEntry,
};
pub use plugin_framework::communication_commands::{
    BuiltinCommunicationCommand, BuiltinCommunicationIntent, CommunicationCommandIntent,
};

#[async_trait]
pub trait CommunicationCommandProviderPort: Send + Sync {
    async fn available(
        &self,
        actor: &ActorContext,
        installation_id: Uuid,
        command_id: &str,
    ) -> Result<bool>;
    async fn freeze_target(
        &self,
        workspace_id: Uuid,
        installation_id: Uuid,
        command_id: &str,
    ) -> Result<FrozenCommunicationCommandTarget>;
}

impl<R> NotificationService<R>
where
    R: NotificationRepository + PluginRepository + RoleConsolePolicyReader,
{
    async fn command_entries(
        &self,
        actor: &ActorContext,
        settings: &[CommunicationCommandSetting],
    ) -> Result<Vec<CommunicationCommandCatalogEntry>> {
        self.ensure_actor(actor).await?;
        let assigned = self
            .repository
            .list_assignments(actor.current_workspace_id)
            .await?
            .into_iter()
            .map(|assignment| assignment.installation_id)
            .collect::<BTreeSet<_>>();
        let mut plugins = Vec::new();
        for installation in self.repository.list_installations().await? {
            if !assigned.contains(&installation.id) {
                continue;
            }
            let Some(raw) = installation.metadata_json.get("communication_commands") else {
                continue;
            };
            let descriptors: Vec<extension_contracts::CommunicationCommandDescriptor> =
                serde_json::from_value(raw.clone())?;
            for descriptor in descriptors {
                descriptor.validate_plugin_namespace(
                    &installation.organization,
                    &installation.provider_code,
                )?;
                let available = installation.desired_state
                    == domain::PluginDesiredState::ActiveRequested
                    && match &self.commands {
                        Some(provider) => provider
                            .available(actor, installation.id, &descriptor.command_id)
                            .await
                            .unwrap_or(false),
                        None => false,
                    };
                plugins.push(CommunicationCommandCatalogEntry {
                    enabled: descriptor.default_enabled,
                    descriptor,
                    source_name: installation.provider_code.clone(),
                    installation_id: Some(installation.id),
                    available,
                    unavailable_reason: (!available).then(|| "plugin_unavailable".into()),
                    builtin: None,
                });
            }
        }
        Ok(effective_communication_commands(
            plugins,
            &settings
                .iter()
                .map(|setting| (setting.command_id.clone(), setting.enabled))
                .collect::<Vec<_>>(),
        )
        .map_err(|_| Error::InvalidInput("communication_command_settings"))?)
    }

    pub async fn command_catalog(
        &self,
        actor: &ActorContext,
        settings: &[CommunicationCommandSetting],
    ) -> Result<Vec<CommunicationCommandView>> {
        let entries = self.command_entries(actor, settings).await?;
        let mut views = entries
            .into_iter()
            .map(|entry| CommunicationCommandView {
                command_id: entry.descriptor.command_id,
                name: entry.descriptor.name,
                description: entry.descriptor.description,
                usage: entry.descriptor.usage,
                source_kind: if entry.builtin.is_some() {
                    CommunicationCommandSourceKind::Builtin
                } else {
                    CommunicationCommandSourceKind::Plugin
                },
                source_name: entry.source_name,
                installation_id: entry.installation_id,
                enabled: entry.enabled,
                available: entry.available,
                unavailable_reason: entry.unavailable_reason,
            })
            .collect::<Vec<_>>();
        // Removed plugins keep their saved setting visible and disableable, without an invented target.
        for setting in settings {
            if !views
                .iter()
                .any(|entry| entry.command_id == setting.command_id)
            {
                views.push(CommunicationCommandView {
                    command_id: setting.command_id.clone(),
                    name: setting.command_id.clone(),
                    description: "Command declaration is no longer installed".into(),
                    usage: String::new(),
                    source_kind: CommunicationCommandSourceKind::Plugin,
                    source_name: setting
                        .command_id
                        .split('/')
                        .take(2)
                        .collect::<Vec<_>>()
                        .join("/"),
                    installation_id: None,
                    enabled: setting.enabled,
                    available: false,
                    unavailable_reason: Some("command_not_installed".into()),
                });
            }
        }
        Ok(views)
    }

    pub async fn resolve_command(
        &self,
        actor: &ActorContext,
        settings: &[CommunicationCommandSetting],
        text: &str,
    ) -> Result<CommunicationCommandIntent> {
        Ok(parse_communication_command(
            text,
            &self.command_entries(actor, settings).await?,
        ))
    }

    pub(super) async fn validate_command_settings(
        &self,
        actor: &ActorContext,
        settings: &[CommunicationCommandSetting],
        prior: &[CommunicationCommandSetting],
    ) -> Result<()> {
        let catalog = self.command_catalog(actor, settings).await?;
        for entry in catalog {
            if entry.unavailable_reason.as_deref() == Some("command_not_installed")
                && !prior.iter().any(|s| s.command_id == entry.command_id)
            {
                return Err(Error::InvalidInput("unknown_communication_command_setting").into());
            }
            if entry.enabled
                && matches!(
                    entry.unavailable_reason.as_deref(),
                    Some("command_name_conflict" | "duplicate_command_identity")
                )
            {
                return Err(Error::Conflict("communication_command_conflict").into());
            }
        }
        Ok(())
    }
}

/// The host supplies the selected application's existing model capability decision.
pub fn apply_communication_model_availability(
    commands: &mut [CommunicationCommandView],
    unavailable_reason: Option<String>,
) {
    for command in commands
        .iter_mut()
        .filter(|command| command.command_id == "host/model")
    {
        command.available = unavailable_reason.is_none();
        command.unavailable_reason = unavailable_reason.clone();
    }
}

pub fn communication_command_help(
    commands: &[CommunicationCommandView],
) -> Result<CommunicationCommandReply, CommunicationCommandReplyContentError> {
    let items = commands
        .iter()
        .filter(|command| command.enabled && command.available)
        .map(|command| {
            vec![
                Inline::Code(command.usage.clone()),
                Inline::Text(format!(" — {}", command.description)),
            ]
        })
        .collect::<Vec<_>>();
    if items.is_empty() {
        return CommunicationCommandReply::from_content("No commands are currently enabled.");
    }
    communication_command_list_reply(Vec::new(), items)
}

/// Host catalog replies omit complete trailing entries when the wire budget is exceeded.
/// The shared renderer owns Markdown syntax; this owner only chooses the visible entries.
pub fn communication_command_list_reply(
    prefix: Vec<Block>,
    items: Vec<Vec<Inline>>,
) -> Result<CommunicationCommandReply, CommunicationCommandReplyContentError> {
    let render = |count: usize, truncated: bool| {
        let mut blocks = prefix.clone();
        if count > 0 {
            blocks.push(Block::BulletList(items[..count].to_vec()));
        }
        if truncated {
            blocks.push(Block::Paragraph(vec![Inline::Text(
                "… Additional entries are available in channel settings.".into(),
            )]));
        }
        CommunicationCommandReply::from_content(Content::Blocks(blocks))
    };
    match render(items.len(), false) {
        Ok(reply) => return Ok(reply),
        Err(CommunicationCommandReplyContentError::TooLong { .. }) if !items.is_empty() => {}
        Err(error) => return Err(error),
    }
    let mut best = render(0, true)?;
    let (mut low, mut high) = (0, items.len());
    while low + 1 < high {
        let count = low + (high - low) / 2;
        match render(count, true) {
            Ok(reply) => {
                best = reply;
                low = count;
            }
            Err(CommunicationCommandReplyContentError::TooLong { .. }) => high = count,
            Err(error) => return Err(error),
        }
    }
    Ok(best)
}

#[cfg(test)]
#[path = "_tests/command_catalog.rs"]
mod tests;
