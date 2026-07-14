use anyhow::{Result, anyhow, bail};
use control_plane::ports::RoleConsolePolicyMigrationRepository;
use uuid::Uuid;

use crate::config::ApiConfig;

use super::{
    live::{load_live_context, preview_live_migration},
    report::{ConsolePolicyMigrationCutoverEvidence, ConsolePolicyMigrationEvidenceReport},
    write_evidence_report,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsolePolicyMigrationCommand {
    Preview,
    Apply,
    Finalize,
    Rollback,
}

impl ConsolePolicyMigrationCommand {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Apply => "apply",
            Self::Finalize => "finalize",
            Self::Rollback => "rollback",
        }
    }
}

pub fn parse_command(value: &str) -> Result<ConsolePolicyMigrationCommand> {
    match value {
        "preview" => Ok(ConsolePolicyMigrationCommand::Preview),
        "apply" => Ok(ConsolePolicyMigrationCommand::Apply),
        "finalize" => Ok(ConsolePolicyMigrationCommand::Finalize),
        "rollback" => Ok(ConsolePolicyMigrationCommand::Rollback),
        _ => bail!("unknown command {value}; expected preview, apply, finalize, or rollback"),
    }
}

struct CommandArguments {
    command: ConsolePolicyMigrationCommand,
    run_id: Uuid,
    actor_user_id: Option<Uuid>,
}

pub async fn run_from_env() -> Result<()> {
    let arguments = parse_arguments(std::env::args().skip(1).collect())?;
    let config = ApiConfig::from_env()?;
    let context = load_live_context(&config).await?;
    let cutover_before = context
        .store
        .role_console_policy_migration_cutover_state()
        .await?;
    let preview =
        preview_live_migration(&context.store, &context.migration, arguments.run_id).await?;
    let mut report = ConsolePolicyMigrationEvidenceReport::for_compiled(
        arguments.command.as_str(),
        arguments.run_id.to_string(),
        &context.migration,
    );
    report.role_projections = preview.role_projections;
    report.actor_multi_role_five_probe_matrix = preview.actor_previews;
    report.unknown_grants = preview.unknown_grants;
    report.authorization_deltas = preview.authorization_deltas;
    report.validation_errors = preview.validation_errors;
    report.cutover_before = Some(cutover_before.into());

    if !report.validation_errors.is_empty() {
        report.cutover_after = report.cutover_before.clone();
        let paths = write_evidence_report(&report)?;
        bail!(
            "migration preview failed validation; evidence written to {} and {}",
            paths.json.display(),
            paths.markdown.display()
        );
    }

    match arguments.command {
        ConsolePolicyMigrationCommand::Preview => {}
        ConsolePolicyMigrationCommand::Apply => {
            let actor_user_id = required_actor(arguments.actor_user_id, arguments.command)?;
            let rehearsal = preview
                .rehearsal
                .ok_or_else(|| anyhow!("valid migration preview has no rehearsal artifact"))?;
            context
                .store
                .rehearse_role_console_policy_migration(&rehearsal)
                .await?;
            context
                .store
                .apply_role_console_policy_migration(&rehearsal, actor_user_id)
                .await?;
        }
        ConsolePolicyMigrationCommand::Finalize => {
            let actor_user_id = required_actor(arguments.actor_user_id, arguments.command)?;
            context
                .store
                .finalize_role_console_policy_migration(arguments.run_id, actor_user_id)
                .await?;
        }
        ConsolePolicyMigrationCommand::Rollback => {
            let actor_user_id = required_actor(arguments.actor_user_id, arguments.command)?;
            context
                .store
                .rollback_role_console_policy_migration(arguments.run_id, actor_user_id)
                .await?;
        }
    }

    let cutover_after = context
        .store
        .role_console_policy_migration_cutover_state()
        .await?;
    report.cutover_after = Some(ConsolePolicyMigrationCutoverEvidence::from(cutover_after));
    let paths = write_evidence_report(&report)?;
    println!(
        "console-policy migration {} evidence: {} {}",
        arguments.command.as_str(),
        paths.json.display(),
        paths.markdown.display()
    );
    Ok(())
}

fn required_actor(
    actor_user_id: Option<Uuid>,
    command: ConsolePolicyMigrationCommand,
) -> Result<Uuid> {
    actor_user_id.ok_or_else(|| anyhow!("{} requires --actor-user-id <uuid>", command.as_str()))
}

fn parse_arguments(arguments: Vec<String>) -> Result<CommandArguments> {
    let Some(command) = arguments.first() else {
        bail!(
            "usage: console_policy_migration <preview|apply|finalize|rollback> [--run-id <uuid>] [--actor-user-id <uuid>]"
        );
    };
    let command = parse_command(command)?;
    let mut run_id = None;
    let mut actor_user_id = None;
    let mut index = 1;
    while index < arguments.len() {
        let flag = &arguments[index];
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| anyhow!("{flag} requires a UUID value"))?;
        let parsed = Uuid::parse_str(value).map_err(|_| anyhow!("{flag} must be a UUID"))?;
        match flag.as_str() {
            "--run-id" => run_id = Some(parsed),
            "--actor-user-id" => actor_user_id = Some(parsed),
            _ => bail!("unknown argument {flag}"),
        }
        index += 2;
    }
    if matches!(
        command,
        ConsolePolicyMigrationCommand::Finalize | ConsolePolicyMigrationCommand::Rollback
    ) && run_id.is_none()
    {
        bail!("{} requires --run-id <uuid>", command.as_str());
    }
    Ok(CommandArguments {
        command,
        run_id: run_id.unwrap_or_else(Uuid::now_v7),
        actor_user_id,
    })
}
