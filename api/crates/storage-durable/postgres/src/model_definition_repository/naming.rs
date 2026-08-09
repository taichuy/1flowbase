use crate::physical_schema_repository::sanitize_identifier_fragment;
use anyhow::{anyhow, Result};
use rand::RngExt as _;
use regex_syntax::hir::{Class, Hir, HirKind};
use uuid::Uuid;

const POSTGRES_IDENTIFIER_MAX_BYTES: usize = 63;
const REGEX_UNBOUNDED_REPEAT_LIMIT: u32 = 16;

#[derive(Clone, Debug, Default)]
pub struct RuntimeTableNamePolicy {
    prefix_generator: Option<rand_regex::Regex>,
}

impl RuntimeTableNamePolicy {
    pub fn from_config(auto_prefix_enabled: bool, prefix_regex: &str) -> Result<Self> {
        if !auto_prefix_enabled {
            return Ok(Self::default());
        }

        let hir = regex_syntax::ParserBuilder::new()
            .unicode(false)
            .utf8(true)
            .build()
            .parse(prefix_regex)
            .map_err(|error| anyhow!("invalid runtime table prefix regex: {error}"))?;
        if !hir_uses_identifier_characters(&hir) {
            return Err(anyhow!(
                "runtime table prefix regex may only generate lowercase ASCII letters, digits, and underscores"
            ));
        }
        let Some((minimum_bytes, maximum_bytes)) = hir_length_bounds(&hir) else {
            return Err(anyhow!(
                "runtime table prefix regex length exceeds supported bounds"
            ));
        };
        if minimum_bytes == 0 {
            return Err(anyhow!(
                "runtime table prefix regex must not generate an empty prefix"
            ));
        }
        if maximum_bytes
            .checked_add(2)
            .is_none_or(|length| length > POSTGRES_IDENTIFIER_MAX_BYTES)
        {
            return Err(anyhow!(
                "runtime table prefix regex can generate a prefix that is too long"
            ));
        }
        let prefix_generator = rand_regex::Regex::with_hir(hir, REGEX_UNBOUNDED_REPEAT_LIMIT)
            .map_err(|error| anyhow!("invalid runtime table prefix regex: {error}"))?;

        Ok(Self {
            prefix_generator: Some(prefix_generator),
        })
    }

    pub fn auto_prefix_enabled(&self) -> bool {
        self.prefix_generator.is_some()
    }

    fn build_table_name(&self, code: &str) -> Result<String> {
        if code.is_empty()
            || !code
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
        {
            return Err(anyhow!(
                "runtime table code must contain only lowercase ASCII letters, digits, and underscores"
            ));
        }

        let physical_table_name = match &self.prefix_generator {
            Some(generator) => {
                let prefix = rand::rng().sample::<String, _>(generator);
                format!("{prefix}_{code}")
            }
            None => code.to_string(),
        };
        if physical_table_name.len() > POSTGRES_IDENTIFIER_MAX_BYTES {
            return Err(anyhow!(
                "runtime physical table name exceeds PostgreSQL's 63-byte identifier limit"
            ));
        }

        Ok(physical_table_name)
    }
}

fn hir_uses_identifier_characters(hir: &Hir) -> bool {
    match hir.kind() {
        HirKind::Empty => true,
        HirKind::Literal(literal) => literal.0.iter().copied().all(is_identifier_byte),
        HirKind::Class(Class::Bytes(class)) => class
            .iter()
            .all(|range| (range.start()..=range.end()).all(is_identifier_byte)),
        HirKind::Class(Class::Unicode(class)) => class.iter().all(|range| {
            range.end().is_ascii()
                && (range.start() as u32..=range.end() as u32)
                    .all(|value| char::from_u32(value).is_some_and(is_identifier_char))
        }),
        HirKind::Look(_) => false,
        HirKind::Repetition(repetition) => hir_uses_identifier_characters(&repetition.sub),
        HirKind::Capture(capture) => hir_uses_identifier_characters(&capture.sub),
        HirKind::Concat(parts) | HirKind::Alternation(parts) => {
            parts.iter().all(hir_uses_identifier_characters)
        }
    }
}

fn hir_length_bounds(hir: &Hir) -> Option<(usize, usize)> {
    match hir.kind() {
        HirKind::Empty | HirKind::Look(_) => Some((0, 0)),
        HirKind::Literal(literal) => Some((literal.0.len(), literal.0.len())),
        HirKind::Class(_) => Some((1, 1)),
        HirKind::Repetition(repetition) => {
            let (part_minimum, part_maximum) = hir_length_bounds(&repetition.sub)?;
            let minimum = part_minimum.checked_mul(repetition.min as usize)?;
            let maximum_repetitions = repetition
                .max
                .unwrap_or(repetition.min.checked_add(REGEX_UNBOUNDED_REPEAT_LIMIT)?);
            let maximum = part_maximum.checked_mul(maximum_repetitions as usize)?;
            Some((minimum, maximum))
        }
        HirKind::Capture(capture) => hir_length_bounds(&capture.sub),
        HirKind::Concat(parts) => parts.iter().try_fold((0usize, 0usize), |bounds, part| {
            let part_bounds = hir_length_bounds(part)?;
            Some((
                bounds.0.checked_add(part_bounds.0)?,
                bounds.1.checked_add(part_bounds.1)?,
            ))
        }),
        HirKind::Alternation(parts) => {
            let mut parts = parts.iter();
            let mut bounds = hir_length_bounds(parts.next()?)?;
            for part in parts {
                let part_bounds = hir_length_bounds(part)?;
                bounds = (bounds.0.min(part_bounds.0), bounds.1.max(part_bounds.1));
            }
            Some(bounds)
        }
    }
}

fn is_identifier_byte(value: u8) -> bool {
    value.is_ascii_lowercase() || value.is_ascii_digit() || value == b'_'
}

fn is_identifier_char(value: char) -> bool {
    value.is_ascii_lowercase() || value.is_ascii_digit() || value == '_'
}

pub(super) fn registered_system_table_name(
    scope_kind: domain::DataModelScopeKind,
    source_kind: domain::DataModelSourceKind,
    protection: &domain::DataModelProtection,
    code: &str,
) -> Option<&'static str> {
    if scope_kind != domain::DataModelScopeKind::System
        || source_kind != domain::DataModelSourceKind::MainSource
        || protection.owner_kind != domain::DataModelOwnerKind::Core
        || !protection.is_protected
    {
        return None;
    }

    domain::builtin_data_model_contract(code).map(|contract| contract.physical_table_name)
}

pub(super) fn is_registered_system_table(model: &domain::ModelDefinitionRecord) -> bool {
    registered_system_table_name(
        model.scope_kind,
        model.source_kind,
        &model.protection,
        &model.code,
    )
    .is_some()
}

pub(super) fn build_physical_table_name(
    policy: &RuntimeTableNamePolicy,
    code: &str,
) -> Result<String> {
    policy.build_table_name(code)
}

pub(super) fn build_physical_column_name(code: &str) -> String {
    sanitize_identifier_fragment(code)
}

pub(super) fn nullable_actor_user_id(actor_user_id: Uuid) -> Option<Uuid> {
    (!actor_user_id.is_nil()).then_some(actor_user_id)
}
