//! User slash commands publish workspace events through an explicit Host target.
use super::*;
use crate::ports::EventCenterRepository;
use domain::event_center::EventDefinitionRecord;
use domain::{
    CommunicationCommandSetting, CommunicationCommandSourceKind, CommunicationCommandView,
    CommunicationEventCommandBinding,
};

fn validate_declarations(
    bindings: &[CommunicationEventCommandBinding],
    commands: &[CommunicationCommandView],
) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut names = commands
        .iter()
        .map(|c| c.name.as_str())
        .collect::<BTreeSet<_>>();
    for binding in bindings {
        let slug = binding.command_id.strip_prefix("user/").unwrap_or_default();
        let token = |s: &str| {
            !s.is_empty()
                && s.len() <= 64
                && s.bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
        };
        if !token(slug)
            || !extension_contracts::communication_command_name_is_valid(&binding.name)
            || extension_contracts::communication_command_name_is_reserved(&binding.name)
            || binding.description.trim().is_empty()
            || binding.description.len() > 4096
            || !ids.insert(&binding.command_id)
            || !names.insert(&binding.name)
        {
            return Err(Error::InvalidInput("invalid_event_command_declaration").into());
        }
        if !binding.contract_id.starts_with("custom.") || binding.contract_version.trim().is_empty()
        {
            return Err(Error::InvalidInput("invalid_event_command_target").into());
        }
        for (field, pointer) in &binding.input_mapping {
            if field.trim().is_empty()
                || !matches!(
                    pointer.as_str(),
                    "/arguments" | "/input_id" | "/conversation_id" | "/instance_id"
                )
            {
                return Err(Error::InvalidInput("invalid_event_command_mapping").into());
            }
        }
    }
    Ok(())
}

fn definition<'a>(
    binding: &CommunicationEventCommandBinding,
    definitions: &'a [EventDefinitionRecord],
    workspace_id: Uuid,
) -> Option<&'a EventDefinitionRecord> {
    definitions.iter().find(|d| {
        d.workspace_id == workspace_id
            && d.enabled
            && d.contract_id == binding.contract_id
            && d.contract_version == binding.contract_version
    })
}

/// Every supported context member is a string. Validate mappings against the current schema
/// while saving, then validate the actual argument value again before publication.
pub fn communication_event_payload(
    binding: &CommunicationEventCommandBinding,
    context: &Value,
    definition: &EventDefinitionRecord,
) -> Result<Value> {
    anyhow::ensure!(
        binding.enabled
            && definition.enabled
            && binding.contract_id == definition.contract_id
            && binding.contract_version == definition.contract_version,
        "event command target unavailable"
    );
    let mut payload = serde_json::Map::new();
    for (field, pointer) in &binding.input_mapping {
        anyhow::ensure!(
            matches!(
                pointer.as_str(),
                "/arguments" | "/input_id" | "/conversation_id" | "/instance_id"
            ),
            "invalid event command mapping"
        );
        payload.insert(
            field.clone(),
            context
                .pointer(pointer)
                .cloned()
                .ok_or(Error::InvalidInput("event_command_context_missing"))?,
        );
    }
    let payload = Value::Object(payload);
    let validator = jsonschema::options()
        .offline()
        .build(&definition.payload_schema)
        .map_err(|_| Error::InvalidInput("event_command_schema_invalid"))?;
    if !validator.is_valid(&payload) {
        return Err(Error::InvalidInput("event_command_payload_invalid").into());
    }
    Ok(payload)
}

fn validate_target_mapping(
    binding: &CommunicationEventCommandBinding,
    target: &EventDefinitionRecord,
) -> Result<()> {
    // Do not reject schemas containing string pattern/enum constraints based on a fabricated argument.
    // Required fields and property types are checked structurally; actual values are validated on use.
    let properties = target
        .payload_schema
        .get("properties")
        .and_then(Value::as_object);
    if target
        .payload_schema
        .get("required")
        .and_then(Value::as_array)
        .is_some_and(|required| {
            required.iter().any(|field| {
                field
                    .as_str()
                    .is_none_or(|field| !binding.input_mapping.contains_key(field))
            })
        })
        || binding.input_mapping.keys().any(|field| {
            properties.is_some_and(|props| !props.contains_key(field))
                && target.payload_schema.get("additionalProperties") == Some(&Value::Bool(false))
        })
    {
        return Err(Error::InvalidInput("invalid_event_command_mapping").into());
    }
    if target
        .payload_schema
        .get("type")
        .is_some_and(|kind| match kind {
            Value::String(kind) => kind != "object",
            Value::Array(kinds) => !kinds.iter().any(|kind| kind == "object"),
            _ => true,
        })
        || binding.input_mapping.keys().any(|field| {
            properties
                .and_then(|props| props.get(field))
                .and_then(|property| property.get("type"))
                .is_some_and(|kind| match kind {
                    Value::String(kind) => kind != "string",
                    Value::Array(kinds) => !kinds.iter().any(|kind| kind == "string"),
                    _ => true,
                })
        })
    {
        return Err(Error::InvalidInput("invalid_event_command_mapping").into());
    }
    jsonschema::options()
        .offline()
        .build(&target.payload_schema)
        .map_err(|_| Error::InvalidInput("event_command_schema_invalid"))?;
    Ok(())
}

impl<R> NotificationService<R>
where
    R: NotificationRepository + PluginRepository + RoleConsolePolicyReader + EventCenterRepository,
{
    pub async fn validate_event_commands(
        &self,
        actor: &ActorContext,
        settings: &[CommunicationCommandSetting],
        bindings: &[CommunicationEventCommandBinding],
    ) -> Result<()> {
        let commands = self.command_catalog(actor, settings).await?;
        validate_declarations(bindings, &commands)?;
        let definitions = self
            .repository
            .list_event_definitions(actor.current_workspace_id)
            .await?;
        for binding in bindings {
            let target = definition(binding, &definitions, actor.current_workspace_id)
                .ok_or(Error::InvalidInput("event_command_target_unavailable"))?;
            validate_target_mapping(binding, target)?;
        }
        Ok(())
    }

    pub async fn event_command_catalog(
        &self,
        actor: &ActorContext,
        settings: &[CommunicationCommandSetting],
        bindings: &[CommunicationEventCommandBinding],
    ) -> Result<Vec<CommunicationCommandView>> {
        let mut commands = self.command_catalog(actor, settings).await?;
        let declarations_valid = validate_declarations(bindings, &commands).is_ok();
        let definitions = self
            .repository
            .list_event_definitions(actor.current_workspace_id)
            .await?;
        for command in &mut commands {
            if bindings.iter().any(|b| b.name == command.name) {
                command.available = false;
                command.unavailable_reason = Some("command_name_conflict".into());
            }
        }
        for binding in bindings {
            let available = declarations_valid
                && definition(binding, &definitions, actor.current_workspace_id)
                    .is_some_and(|target| validate_target_mapping(binding, target).is_ok());
            commands.push(CommunicationCommandView {
                command_id: binding.command_id.clone(),
                name: binding.name.clone(),
                description: binding.description.clone(),
                usage: format!("/{} [arguments]", binding.name),
                source_kind: CommunicationCommandSourceKind::User,
                source_name: "user".into(),
                installation_id: None,
                enabled: binding.enabled,
                available,
                unavailable_reason: (!available).then(|| "event_command_target_unavailable".into()),
            });
        }
        Ok(commands)
    }

    pub async fn resolve_event_command(
        &self,
        actor: &ActorContext,
        settings: &[CommunicationCommandSetting],
        bindings: &[CommunicationEventCommandBinding],
        text: &str,
    ) -> Result<CommunicationCommandIntent> {
        let text = text.trim();
        let (token, arguments) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
        let matching = bindings
            .iter()
            .filter(|b| token.strip_prefix('/') == Some(b.name.as_str()))
            .collect::<Vec<_>>();
        if matching.is_empty() {
            return self.resolve_command(actor, settings, text).await;
        }
        let catalog = self
            .event_command_catalog(actor, settings, bindings)
            .await?;
        let valid = matching.len() == 1
            && text.len() <= 16_384
            && catalog
                .iter()
                .filter(|c| format!("/{}", c.name) == token)
                .count()
                == 1;
        if !valid
            || !matching[0].enabled
            || !catalog
                .iter()
                .any(|c| c.command_id == matching[0].command_id && c.available)
        {
            return Ok(CommunicationCommandIntent::Rejected {
                code: "event_command_unavailable".into(),
                reply: "This event command is disabled or unavailable. Check channel settings."
                    .into(),
            });
        }
        Ok(CommunicationCommandIntent::Event {
            command_id: matching[0].command_id.clone(),
            arguments: arguments.trim().into(),
        })
    }

    pub async fn event_command_definition(
        &self,
        actor: &ActorContext,
        binding: &CommunicationEventCommandBinding,
    ) -> Result<EventDefinitionRecord> {
        self.ensure_actor(actor).await?;
        let definitions = self
            .repository
            .list_event_definitions(actor.current_workspace_id)
            .await?;
        definition(binding, &definitions, actor.current_workspace_id)
            .cloned()
            .ok_or_else(|| Error::InvalidInput("event_command_target_unavailable").into())
    }
}

#[cfg(test)]
#[path = "_tests/event_commands.rs"]
mod tests;
