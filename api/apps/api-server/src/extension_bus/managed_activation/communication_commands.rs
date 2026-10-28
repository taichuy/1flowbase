use super::*;
use control_plane_contracts::ports::{
    CompleteManagedCommunicationCommand, FrozenCommunicationCommandTarget,
    FrozenLifecyclePublication, LifecyclePublicationPlan, LifecycleSubscriberTarget,
};
use extension_contracts::{
    CommunicationCommandDescriptor, CommunicationCommandReply, ManagedEventOutcome,
    COMMUNICATION_COMMAND_POINT,
};
use plugin_framework::communication_commands::{
    effective_communication_commands, select_communication_command_subscriber,
    CommunicationCommandCatalogEntry,
};

pub(crate) struct ManagedCommunicationCommandProvider {
    composition: Arc<ManagedExtensionComposition>,
}
impl ManagedCommunicationCommandProvider {
    pub(crate) fn new(composition: Arc<ManagedExtensionComposition>) -> Self {
        Self { composition }
    }
}

fn declarations(
    snapshot: &ManagedWorkspaceSnapshot,
) -> Result<Vec<CommunicationCommandCatalogEntry>> {
    let installations = snapshot
        .bindings
        .values()
        .map(|binding| (binding.installation.id, &binding.installation))
        .collect::<BTreeMap<_, _>>();
    let mut commands = Vec::new();
    for (installation_id, installation) in installations {
        let Some(raw) = installation.metadata_json.get("communication_commands") else {
            continue;
        };
        let descriptors: Vec<CommunicationCommandDescriptor> = serde_json::from_value(raw.clone())?;
        for descriptor in descriptors {
            descriptor.validate_plugin_namespace(
                &installation.organization,
                &installation.provider_code,
            )?;
            let available = snapshot
                .bindings
                .get(&descriptor.contribution_id)
                .is_some_and(|binding| {
                    binding.installation.id == installation_id
                        && binding.descriptor.point_id.as_str() == COMMUNICATION_COMMAND_POINT
                });
            commands.push(CommunicationCommandCatalogEntry {
                enabled: descriptor.default_enabled,
                descriptor,
                source_name: installation.provider_code.clone(),
                installation_id: Some(installation_id),
                available,
                unavailable_reason: (!available).then(|| "command_binding_unavailable".into()),
                builtin: None,
            });
        }
    }
    Ok(commands)
}

fn descriptor(
    snapshot: &ManagedWorkspaceSnapshot,
    installation_id: Uuid,
    command_id: &str,
) -> Result<CommunicationCommandDescriptor> {
    let matches = declarations(snapshot)?
        .into_iter()
        .filter(|entry| {
            entry.installation_id == Some(installation_id)
                && entry.descriptor.command_id == command_id
                && entry.available
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        matches.len() == 1,
        "command requires one active declaration"
    );
    Ok(matches
        .into_iter()
        .next()
        .expect("one declaration")
        .descriptor)
}

#[async_trait::async_trait]
impl control_plane::notifications::CommunicationCommandProviderPort
    for ManagedCommunicationCommandProvider
{
    async fn available(
        &self,
        actor: &domain::ActorContext,
        installation_id: Uuid,
        command_id: &str,
    ) -> Result<bool> {
        Ok(self
            .freeze_target(actor.current_workspace_id, installation_id, command_id)
            .await
            .is_ok())
    }
    async fn freeze_target(
        &self,
        workspace_id: Uuid,
        installation_id: Uuid,
        command_id: &str,
    ) -> Result<FrozenCommunicationCommandTarget> {
        let snapshot = self
            .composition
            .snapshot(workspace_id)
            .await
            .context("command workspace inactive")?;
        let descriptor = descriptor(&snapshot, installation_id, command_id)?;
        let binding = snapshot
            .bindings
            .get(&descriptor.contribution_id)
            .context("command binding missing")?;
        let lease = self
            .composition
            .store
            .lock_contribution_authority(binding.handle.identity().subject())
            .await?;
        self.composition
            .validate_event_current_binding(binding, workspace_id, lease.as_ref())?;
        let subscriber = select_communication_command_subscriber(
            snapshot
                .lifecycle_plan
                .as_ref()
                .context("command lifecycle plan missing")?,
            &descriptor.contribution_id,
        )?;
        let reference = snapshot.freeze_reference()?;
        let publication = FrozenLifecyclePublication::managed(
            LifecyclePublicationPlan {
                graph_fingerprint: snapshot.graph.fingerprint().as_str().into(),
                subscribers: vec![LifecycleSubscriberTarget {
                    subscriber_id: subscriber.subscriber_id.clone(),
                    handler_id: subscriber.handler_id.clone(),
                    handler_version: subscriber.handler_version.clone(),
                }],
            },
            Box::new(reference),
        );
        lease.release().await?;
        Ok(FrozenCommunicationCommandTarget {
            subject: binding.handle.identity().subject().clone(),
            publication,
        })
    }
}

impl ManagedExtensionComposition {
    /// Frozen graph loss and revoked authority can reject execution, but cannot prevent the
    /// Host from resolving the persisted request and releasing its ordinary-input barrier.
    pub(crate) async fn terminate_command_delivery(
        &self,
        fact: &control_plane_contracts::ports::LifecycleOutboxRecord,
        termination: control_plane_contracts::ports::CommunicationCommandTermination,
    ) -> Result<()> {
        control_plane::notifications::terminate_communication_command_delivery(
            &self.store,
            fact,
            termination,
        )
        .await
    }
    /// Return true only for an already committed result. Current authority is checked by the
    /// caller before this receipt shortcut, so revocation never becomes a replay authorization.
    pub(super) async fn validate_command_delivery(
        &self,
        _frozen: &ManagedWorkspaceSnapshot,
        binding: &ManagedContributionBinding,
        record: &control_plane_contracts::ports::LifecycleOutboxRecord,
        lease: &mut dyn ContributionAuthorityLease,
    ) -> Result<bool> {
        if record.contract_id != COMMUNICATION_COMMAND_POINT {
            return Ok(false);
        }
        let workspace_id = Uuid::parse_str(binding.handle.identity().workspace_id().as_str())?;
        let context = lease
            .load_communication_command_context(
                binding.handle.identity().subject(),
                record.event_id,
            )
            .await?
            .context("command request receipt missing")?;
        let request = context.command;
        anyhow::ensure!(
            request.installation_id == Some(binding.installation.id)
                && request.contribution_id.as_deref()
                    == Some(binding.descriptor.contribution_id.as_str())
                && request.subscriber_id.as_deref() == Some(record.subscriber_id.as_str())
                && request
                    .target
                    .as_ref()
                    .is_some_and(
                        |target| target.graph_fingerprint == record.graph_fingerprint
                            && target.handler_id == record.handler_id
                            && target.handler_version == record.handler_version
                    ),
            "command receipt frozen target mismatch"
        );
        let delivered: extension_contracts::ManagedEventDelivery =
            serde_json::from_slice(&record.canonical_payload)?;
        anyhow::ensure!(
            delivered.payload == serde_json::to_value(&request.request)?
                && delivered.workspace_id == request.scope.workspace_id.to_string()
                && delivered.causation_id == request.request.input_id
                && delivered.correlation_id == request.request.input_id,
            "command event does not match persisted request"
        );
        if !context.channel_enabled {
            return Err(control_plane_contracts::ports::LifecycleDeliveryBlocked(
                control_plane_contracts::ports::LifecycleDeliveryPauseReason::AuthorityRevoked,
            )
            .into());
        }
        let saved = context.binding.context("command binding removed")?;
        let current = self
            .snapshot(workspace_id)
            .await
            .context("command workspace disabled")?;
        let entries = effective_communication_commands(
            declarations(&current)?,
            &saved
                .config
                .command_settings
                .iter()
                .map(|setting| (setting.command_id.clone(), setting.enabled))
                .collect::<Vec<_>>(),
        )?;
        let matches = entries
            .iter()
            .filter(|entry| {
                entry.descriptor.command_id == request.request.command_id
                    && entry.installation_id == request.installation_id
                    && entry.enabled
                    && entry.available
            })
            .count();
        if matches != 1 {
            return Err(control_plane_contracts::ports::LifecycleDeliveryBlocked(
                control_plane_contracts::ports::LifecycleDeliveryPauseReason::AuthorityRevoked,
            )
            .into());
        }
        Ok(request.reply.is_some())
    }

    pub(super) async fn commit_command_outcome(
        &self,
        binding: &ManagedContributionBinding,
        record: &control_plane_contracts::ports::LifecycleOutboxRecord,
        outcome: ManagedEventOutcome,
    ) -> Result<()> {
        let (reply, failure_code) = match outcome {
            ManagedEventOutcome::CommandReply { reply } => (reply, None),
            ManagedEventOutcome::Failed { classification } => (
                CommunicationCommandReply {
                    text: format!("Command failed: {classification}"),
                },
                Some(classification),
            ),
            _ => bail!("command requires bounded typed reply or explicit failure"),
        };
        reply.validate()?;
        let workspace_id = Uuid::parse_str(binding.handle.identity().workspace_id().as_str())?;
        let mut lease = self
            .store
            .lock_contribution_authority(binding.handle.identity().subject())
            .await?;
        self.validate_event_current_binding(binding, workspace_id, lease.as_ref())?;
        // Recheck current channel setting and command identity after the worker returned.
        let current = self
            .snapshot(workspace_id)
            .await
            .context("command workspace disabled")?;
        self.validate_command_delivery(&current, binding, record, lease.as_mut())
            .await?;
        lease
            .commit_communication_command_reply(CompleteManagedCommunicationCommand {
                subject: binding.handle.identity().subject().clone(),
                event_id: record.event_id,
                graph_fingerprint: record.graph_fingerprint.clone(),
                target: LifecycleSubscriberTarget {
                    subscriber_id: record.subscriber_id.clone(),
                    handler_id: record.handler_id.clone(),
                    handler_version: record.handler_version.clone(),
                },
                reply,
                failure_code,
            })
            .await?;
        Ok(())
    }
}
