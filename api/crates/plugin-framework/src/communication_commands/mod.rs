//! One registry for host and plugin command declarations; command parsing never executes work.
use extension_contracts::{CommunicationCommandDescriptor, ContributionId};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinCommunicationCommand {
    New,
    Model,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinCommunicationIntent {
    New,
    Model { model: Option<String> },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommunicationCommandIntent {
    Ordinary,
    Builtin(BuiltinCommunicationIntent),
    Plugin {
        command_id: String,
        arguments: String,
        installation_id: Uuid,
    },
    Rejected {
        code: String,
        reply: String,
    },
}

#[derive(Debug, Clone)]
pub struct CommunicationCommandCatalogEntry {
    pub descriptor: CommunicationCommandDescriptor,
    pub source_name: String,
    pub installation_id: Option<Uuid>,
    pub enabled: bool,
    pub available: bool,
    pub unavailable_reason: Option<String>,
    pub builtin: Option<BuiltinCommunicationCommand>,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid communication command catalog: {0}")]
pub struct CommunicationCommandCatalogError(pub &'static str);

pub fn builtin_communication_commands() -> Vec<CommunicationCommandCatalogEntry> {
    [
        (
            BuiltinCommunicationCommand::New,
            "new",
            "Start a new AI session",
            "/new",
        ),
        (
            BuiltinCommunicationCommand::Model,
            "model",
            "Show or select an allowed model",
            "/model [model]",
        ),
        (
            BuiltinCommunicationCommand::Help,
            "help",
            "Show enabled commands",
            "/help",
        ),
    ]
    .into_iter()
    .map(
        |(builtin, name, description, usage)| CommunicationCommandCatalogEntry {
            descriptor: CommunicationCommandDescriptor {
                command_id: format!("host/{name}"),
                name: name.into(),
                description: description.into(),
                usage: usage.into(),
                contribution_id: ContributionId::new(format!("1flowbase.communication.{name}"))
                    .expect("static command identity"),
                default_enabled: true,
            },
            source_name: "1flowbase".into(),
            installation_id: None,
            enabled: true,
            available: true,
            unavailable_reason: None,
            builtin: Some(builtin),
        },
    )
    .collect()
}

/// Unknown persisted overrides are retained by the caller, but never invent executable entries.
pub fn effective_communication_commands(
    plugins: Vec<CommunicationCommandCatalogEntry>,
    settings: &[(String, bool)],
) -> Result<Vec<CommunicationCommandCatalogEntry>, CommunicationCommandCatalogError> {
    let mut overrides = BTreeMap::new();
    for (id, enabled) in settings {
        if id.is_empty() || id.len() > 256 || overrides.insert(id.as_str(), *enabled).is_some() {
            return Err(CommunicationCommandCatalogError(
                "duplicate or invalid setting",
            ));
        }
    }
    for entry in &plugins {
        if entry.builtin.is_some()
            || entry.installation_id.is_none()
            || entry.descriptor.command_id.starts_with("host/")
            || extension_contracts::communication_command_name_is_reserved(&entry.descriptor.name)
            || entry.descriptor.validate().is_err()
        {
            return Err(CommunicationCommandCatalogError(
                "invalid plugin declaration",
            ));
        }
    }
    let mut entries = builtin_communication_commands();
    entries.extend(plugins);
    let mut ids = BTreeMap::<String, usize>::new();
    let mut names = BTreeMap::<String, usize>::new();
    for entry in &mut entries {
        entry.enabled = overrides
            .get(entry.descriptor.command_id.as_str())
            .copied()
            .unwrap_or(entry.descriptor.default_enabled);
        *ids.entry(entry.descriptor.command_id.clone()).or_default() += 1;
        if entry.enabled {
            *names.entry(entry.descriptor.name.clone()).or_default() += 1;
        }
    }
    for entry in &mut entries {
        let reason = if ids[&entry.descriptor.command_id] > 1 {
            Some("duplicate_command_identity")
        } else if entry.enabled && names.get(&entry.descriptor.name).copied().unwrap_or(0) > 1 {
            Some("command_name_conflict")
        } else {
            None
        };
        if let Some(reason) = reason {
            entry.available = false;
            entry.unavailable_reason = Some(reason.into());
        }
    }
    Ok(entries)
}

pub fn parse_communication_command(
    text: &str,
    catalog: &[CommunicationCommandCatalogEntry],
) -> CommunicationCommandIntent {
    let text = text.trim();
    if !text.starts_with('/') {
        return CommunicationCommandIntent::Ordinary;
    }
    let reject = |code: &str, reply: &str| CommunicationCommandIntent::Rejected {
        code: code.into(),
        reply: reply.into(),
    };
    if text.len() > 16_384 {
        return reject("invalid_command", "Command is too long.");
    }
    let (token, arguments) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let mut name = &token[1..];
    let mut arguments = arguments.trim();
    // Opaque model IDs are never interpreted as plugin names or split on punctuation.
    if let Some(model) = name.strip_prefix("model-user-") {
        if model.is_empty() || !arguments.is_empty() {
            return reject("invalid_command", "Use /model or /model-user-<model>.");
        }
        arguments = model;
        name = "model";
    }
    let matching = catalog
        .iter()
        .filter(|entry| entry.descriptor.name == name)
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return reject(
            "unknown_command",
            "Unknown command. Use /help to see available commands.",
        );
    }
    let enabled = matching
        .iter()
        .copied()
        .filter(|entry| entry.enabled)
        .collect::<Vec<_>>();
    if enabled.is_empty() {
        return reject(
            "command_disabled",
            "This command is disabled for this channel.",
        );
    }
    if enabled.len() != 1 || !enabled[0].available {
        return reject(
            "command_unavailable",
            "This command is unavailable. Check channel command settings.",
        );
    }
    let entry = enabled[0];
    if let Some(builtin) = entry.builtin {
        return CommunicationCommandIntent::Builtin(match builtin {
            BuiltinCommunicationCommand::New if arguments.is_empty() => {
                BuiltinCommunicationIntent::New
            }
            BuiltinCommunicationCommand::Help if arguments.is_empty() => {
                BuiltinCommunicationIntent::Help
            }
            BuiltinCommunicationCommand::Model => BuiltinCommunicationIntent::Model {
                model: (!arguments.is_empty()).then(|| arguments.to_string()),
            },
            _ => return reject("invalid_command", "This command does not accept arguments."),
        });
    }
    match entry.installation_id {
        Some(installation_id) => CommunicationCommandIntent::Plugin {
            command_id: entry.descriptor.command_id.clone(),
            arguments: arguments.into(),
            installation_id,
        },
        None => reject(
            "command_unavailable",
            "This command has no execution target.",
        ),
    }
}

/// Select by declaration identity, never by cardinality-Many event fan-out.
pub fn select_communication_command_subscriber<'a>(
    plan: &'a crate::extension_bus::EffectiveLifecycleSubscriberPlan,
    contribution_id: &ContributionId,
) -> Result<&'a crate::extension_bus::EffectiveLifecycleSubscriber, CommunicationCommandCatalogError>
{
    let matches = plan
        .subscribers()
        .iter()
        .filter(|s| {
            s.contribution_id == *contribution_id
                && s.point_id.as_str() == extension_contracts::COMMUNICATION_COMMAND_POINT
                && s.fact_contract_id == extension_contracts::COMMUNICATION_COMMAND_POINT
                && s.fact_contract_version
                    == extension_contracts::COMMUNICATION_COMMAND_CONTRACT_VERSION
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(CommunicationCommandCatalogError(
            "command requires exactly one explicit subscriber",
        ));
    }
    Ok(matches[0])
}

pub fn communication_command_extension_point(
    owner: crate::extension_bus::ModuleId,
) -> Result<
    crate::extension_bus::ExtensionPointDescriptor,
    crate::extension_bus::DescriptorValueError,
> {
    use crate::extension_bus::*;
    let mut contract = ContractDescriptor::new(
        extension_contracts::COMMUNICATION_COMMAND_POINT,
        extension_contracts::COMMUNICATION_COMMAND_CONTRACT_VERSION,
    )?;
    contract.payload_schema =
        Some(extension_contracts::CommunicationCommandRequest::event_schema());
    Ok(ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(extension_contracts::COMMUNICATION_COMMAND_POINT)?,
        owner_module_id: owner,
        point_kind: ExtensionPointKind::EventStream,
        contract,
        scope: ScopeSemantics::Workspace,
        cardinality: Cardinality::Many,
        ordering: OrderingSemantics::Lexicographic,
        failure: FailureSemantics::IsolateContribution,
        delivery: DeliverySemantics::AfterCommitDurable,
        lifecycle: LifecycleSemantics::WorkspaceAssignment,
        allowed_permissions: BTreeSet::from([PermissionCode::new("event.subscribe")?]),
        override_policy: OverridePolicy::Sealed,
    })
}

#[cfg(test)]
mod _tests;
