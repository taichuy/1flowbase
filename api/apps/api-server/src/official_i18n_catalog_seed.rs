use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, bail, Context, Result};
use control_plane::i18n_catalog::VerifiedOfficialCatalogSeed;
use domain::{
    CatalogDigest, CatalogLocale, CatalogMessageIdentity, CatalogModuleId, CatalogSeedFile,
    CatalogVersion, OfficialCatalogMessage, I18N_CATALOG_SEED_SCHEMA_VERSION,
    I18N_CATALOG_SOURCE_LOCALE,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

pub(crate) const OFFICIAL_SEED_BYTES: &[u8] = include_bytes!("../resources/i18n/catalog-seed.json");
pub(crate) const OFFICIAL_SEED_SOURCE_BYTES: &[u8] =
    include_bytes!("../resources/i18n/catalog-seed.source.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogSeed {
    manifest: CatalogSeedManifest,
    modules: Vec<CatalogSeedModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogSeedManifest {
    schema_version: String,
    catalog_version: String,
    source_locale: String,
    locales: Vec<String>,
    modules: Vec<String>,
    files: Vec<CatalogSeedFileDocument>,
    generated_at: String,
    semantic_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogSeedFileDocument {
    module: String,
    locale: String,
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogSeedModule {
    id: String,
    messages: Vec<CatalogSeedMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogSeedMessage {
    msgid: String,
    translations: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogSeedSource {
    catalog_version: String,
    semantic_sha256: String,
    official_commit: String,
    release_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogSeedInspection {
    pub(crate) catalog_version: CatalogVersion,
    pub(crate) semantic_sha256: CatalogDigest,
    pub(crate) seed_sha256: CatalogDigest,
}

pub fn load_official_i18n_catalog_seed() -> Result<VerifiedOfficialCatalogSeed> {
    decode_catalog_seed(OFFICIAL_SEED_BYTES, OFFICIAL_SEED_SOURCE_BYTES)
}

pub(crate) fn decode_catalog_seed(
    seed_bytes: &[u8],
    source_bytes: &[u8],
) -> Result<VerifiedOfficialCatalogSeed> {
    let seed: CatalogSeed =
        serde_json::from_slice(seed_bytes).context("invalid official Seed JSON")?;
    let source: CatalogSeedSource =
        serde_json::from_slice(source_bytes).context("invalid official Seed source metadata")?;
    if source.official_commit != "f0ac7987cbf0b731c106282a62cf9f82070bcb55" {
        bail!("official Seed source commit is not pinned");
    }
    decode_validated_catalog_seed(
        seed,
        source.release_id,
        Some((&source.catalog_version, &source.semantic_sha256)),
    )
}

pub(crate) fn inspect_catalog_seed(seed_bytes: &[u8]) -> Result<CatalogSeedInspection> {
    let seed: CatalogSeed =
        serde_json::from_slice(seed_bytes).context("invalid official Seed JSON")?;
    validate_seed_header(&seed)?;
    validate_seed_digests(&seed)?;
    Ok(CatalogSeedInspection {
        catalog_version: CatalogVersion::new(seed.manifest.catalog_version)?,
        semantic_sha256: CatalogDigest::new(seed.manifest.semantic_sha256)?,
        seed_sha256: CatalogDigest::new(format!("sha256:{:x}", Sha256::digest(seed_bytes)))?,
    })
}

pub(crate) fn decode_downloaded_catalog_seed(
    seed_bytes: &[u8],
    expected: &CatalogSeedInspection,
) -> Result<VerifiedOfficialCatalogSeed> {
    let seed: CatalogSeed =
        serde_json::from_slice(seed_bytes).context("invalid official Seed JSON")?;
    let actual_seed_sha256 = format!("sha256:{:x}", Sha256::digest(seed_bytes));
    if actual_seed_sha256 != expected.seed_sha256.as_str() {
        bail!("official Seed asset checksum mismatch");
    }
    let release_id = release_id_from_digest(&expected.seed_sha256)?;
    decode_validated_catalog_seed(
        seed,
        release_id,
        Some((
            expected.catalog_version.as_str(),
            expected.semantic_sha256.as_str(),
        )),
    )
}

fn decode_validated_catalog_seed(
    seed: CatalogSeed,
    release_id: Uuid,
    expected: Option<(&str, &str)>,
) -> Result<VerifiedOfficialCatalogSeed> {
    validate_seed_header(&seed)?;
    if expected.is_some_and(|(version, semantic_sha256)| {
        seed.manifest.catalog_version != version || seed.manifest.semantic_sha256 != semantic_sha256
    }) {
        bail!("official Seed does not match its fixed release descriptor");
    }
    validate_seed_digests(&seed)?;

    let locales = seed
        .manifest
        .locales
        .iter()
        .cloned()
        .map(CatalogLocale::new)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let modules = seed
        .manifest
        .modules
        .iter()
        .cloned()
        .map(CatalogModuleId::new)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let files = seed
        .manifest
        .files
        .iter()
        .map(|file| {
            CatalogSeedFile::new(
                CatalogModuleId::new(file.module.clone())?,
                CatalogLocale::new(file.locale.clone())?,
                file.path.clone(),
                CatalogDigest::new(file.sha256.clone())?,
            )
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let messages = seed
        .modules
        .iter()
        .flat_map(|module| {
            module.messages.iter().map(|message| {
                let identity = CatalogMessageIdentity::new(
                    CatalogModuleId::new(module.id.clone())?,
                    message.msgid.clone(),
                )?;
                let translations =
                        message
                            .translations
                            .iter()
                            .map(|(locale, translation)| {
                                Ok((CatalogLocale::new(locale.clone())?, translation.clone()))
                            })
                            .collect::<std::result::Result<
                                BTreeMap<_, _>,
                                domain::I18nCatalogInvariantError,
                            >>()?;
                OfficialCatalogMessage::new(identity, translations)
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;

    VerifiedOfficialCatalogSeed::new(
        release_id,
        CatalogVersion::new(seed.manifest.catalog_version)?,
        locales,
        modules,
        files,
        OffsetDateTime::parse(&seed.manifest.generated_at, &Rfc3339)
            .context("official Seed generated_at is not RFC 3339")?,
        CatalogDigest::new(seed.manifest.semantic_sha256)?,
        messages,
    )
    .map_err(Into::into)
}

fn validate_seed_header(seed: &CatalogSeed) -> Result<()> {
    if seed.manifest.schema_version != I18N_CATALOG_SEED_SCHEMA_VERSION {
        bail!("unsupported official Seed schema");
    }
    if seed.manifest.source_locale != I18N_CATALOG_SOURCE_LOCALE {
        bail!("official Seed source locale must be en_US");
    }
    validate_seed_shape(seed)
}

fn release_id_from_digest(digest: &CatalogDigest) -> Result<Uuid> {
    let hexadecimal = digest
        .as_str()
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow!("official Seed digest has no sha256 prefix"))?;
    let mut bytes = [0_u8; 16];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hexadecimal[index * 2..index * 2 + 2], 16)
            .context("official Seed digest is not hexadecimal")?;
    }
    // Stable RFC 9562-compatible identity derived from immutable artifact bytes.
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(Uuid::from_bytes(bytes))
}

fn validate_seed_shape(seed: &CatalogSeed) -> Result<()> {
    let locale_set = seed.manifest.locales.iter().collect::<BTreeSet<_>>();
    let module_set = seed.manifest.modules.iter().collect::<BTreeSet<_>>();
    if locale_set.len() != seed.manifest.locales.len()
        || module_set.len() != seed.manifest.modules.len()
        || module_set.is_empty()
    {
        bail!("official Seed locale/module inventory is not unique and non-empty");
    }
    if seed.modules.len() != module_set.len()
        || seed
            .modules
            .iter()
            .map(|module| &module.id)
            .collect::<BTreeSet<_>>()
            != module_set
    {
        bail!("official Seed module documents do not match the manifest");
    }
    let expected_files = locale_set.len() * module_set.len();
    if seed.manifest.files.len() != expected_files {
        bail!("official Seed file inventory is incomplete");
    }
    let file_pairs = seed
        .manifest
        .files
        .iter()
        .map(|file| (&file.module, &file.locale))
        .collect::<BTreeSet<_>>();
    if file_pairs.len() != expected_files
        || file_pairs
            .iter()
            .any(|(module, locale)| !module_set.contains(module) || !locale_set.contains(locale))
    {
        bail!("official Seed file inventory has an unknown or duplicate module/locale");
    }

    let forbidden = Regex::new(
        r"(?i)</?[A-Za-z][^>]*>|javascript\s*:|\$\{|=>|\bfunction\s*\(|on[A-Za-z]+\s*=|!?\[[^\]]*\]\([^)]+\)",
    )?;
    let named_placeholder = Regex::new(r"\{([A-Za-z_][A-Za-z0-9_.-]*)\}")?;
    for module in &seed.modules {
        let mut identities = BTreeSet::new();
        for message in &module.messages {
            if !identities.insert(&message.msgid) {
                bail!("official Seed contains a duplicate message identity");
            }
            validate_plain_text(&message.msgid, &forbidden, &named_placeholder)?;
            let expected_targets = locale_set
                .iter()
                .filter(|locale| locale.as_str() != I18N_CATALOG_SOURCE_LOCALE)
                .copied()
                .collect::<BTreeSet<_>>();
            if message.translations.keys().collect::<BTreeSet<_>>() != expected_targets {
                bail!("official Seed message target locales do not match the manifest");
            }
            let source_placeholders = placeholders(&message.msgid, &named_placeholder);
            for translation in message.translations.values() {
                validate_plain_text(translation, &forbidden, &named_placeholder)?;
                if placeholders(translation, &named_placeholder) != source_placeholders {
                    bail!("official Seed translation placeholder set does not match its msgid");
                }
            }
        }
    }
    Ok(())
}

fn validate_plain_text(value: &str, forbidden: &Regex, placeholders: &Regex) -> Result<()> {
    if value.is_empty() || forbidden.is_match(value) {
        bail!("official Seed messages must be non-empty plain text");
    }
    let without_placeholders = placeholders.replace_all(value, "");
    if without_placeholders.contains('{') || without_placeholders.contains('}') {
        bail!("official Seed message contains a malformed placeholder");
    }
    Ok(())
}

fn placeholders(value: &str, pattern: &Regex) -> BTreeSet<String> {
    pattern
        .captures_iter(value)
        .map(|capture| capture[1].to_owned())
        .collect()
}

fn validate_seed_digests(seed: &CatalogSeed) -> Result<()> {
    for file in &seed.manifest.files {
        let module = seed
            .modules
            .iter()
            .find(|module| module.id == file.module)
            .ok_or_else(|| anyhow!("official Seed file references an unknown module"))?;
        let document = if file.locale == seed.manifest.source_locale {
            Value::Array(
                module
                    .messages
                    .iter()
                    .map(|message| Value::String(message.msgid.clone()))
                    .collect(),
            )
        } else {
            Value::Object(
                module
                    .messages
                    .iter()
                    .map(|message| {
                        message
                            .translations
                            .get(&file.locale)
                            .cloned()
                            .map(|value| (message.msgid.clone(), Value::String(value)))
                            .ok_or_else(|| anyhow!("official Seed file is missing a translation"))
                    })
                    .collect::<Result<serde_json::Map<_, _>>>()?,
            )
        };
        // The publisher already freezes messages in its locale-aware canonical order. Preserve
        // that order here because Rust's lexical BTree ordering differs for punctuation/case.
        if digest_canonical_json(&document)? != file.sha256 {
            bail!("official Seed file digest mismatch");
        }
    }
    let semantic = json!({
        "catalog_version": seed.manifest.catalog_version,
        "source_locale": seed.manifest.source_locale,
        "locales": seed.manifest.locales,
        "modules": seed.manifest.modules,
        "files": seed.manifest.files,
        "normalized_modules": seed.modules,
    });
    if digest_stable_json(&semantic)? != seed.manifest.semantic_sha256 {
        bail!("official Seed semantic digest mismatch");
    }
    Ok(())
}

fn digest_stable_json(value: &Value) -> Result<String> {
    let sorted = sort_json(value)?;
    digest_canonical_json(&sorted)
}

fn digest_canonical_json(value: &Value) -> Result<String> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn sort_json(value: &Value) -> Result<Value> {
    match value {
        Value::Object(object) => Ok(serde_json::to_value(
            object
                .iter()
                .map(|(key, value)| Ok((key.clone(), sort_json(value)?)))
                .collect::<Result<BTreeMap<_, _>>>()?,
        )?),
        Value::Array(values) => Ok(Value::Array(
            values.iter().map(sort_json).collect::<Result<Vec<_>>>()?,
        )),
        _ => Ok(value.clone()),
    }
}
