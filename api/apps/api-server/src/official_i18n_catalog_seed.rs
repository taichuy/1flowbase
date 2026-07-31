use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, bail, Context, Result};
use control_plane::i18n_catalog::VerifiedOfficialCatalogSeed;
use domain::{
    CatalogDigest, CatalogLocale, CatalogMessageIdentity, CatalogSeedFile, CatalogVersion,
    OfficialCatalogMessage, I18N_CATALOG_SEED_SCHEMA_VERSION, I18N_CATALOG_SOURCE_LOCALE,
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
    messages: Vec<CatalogSeedMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogSeedManifest {
    schema_version: String,
    catalog_version: String,
    source_locale: String,
    locales: Vec<String>,
    files: Vec<CatalogSeedFileDocument>,
    generated_at: String,
    semantic_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogSeedFileDocument {
    keys: Vec<String>,
    locale: String,
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogSeedMessage {
    key: String,
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
    if source.official_commit != "cf9159188bd28ba5d899edfad3d480167f56d187" {
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
    let files = seed
        .manifest
        .files
        .iter()
        .map(|file| {
            CatalogSeedFile::new(
                CatalogLocale::new(file.locale.clone())?,
                file.path.clone(),
                CatalogDigest::new(file.sha256.clone())?,
            )
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let messages = seed
        .messages
        .iter()
        .map(|message| {
            let identity = CatalogMessageIdentity::new(message.key.clone())?;
            let translations = message
                .translations
                .iter()
                .map(|(locale, translation)| {
                    Ok((CatalogLocale::new(locale.clone())?, translation.clone()))
                })
                .collect::<std::result::Result<BTreeMap<_, _>, domain::I18nCatalogInvariantError>>(
                )?;
            OfficialCatalogMessage::new(identity, translations)
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;

    VerifiedOfficialCatalogSeed::new(
        release_id,
        CatalogVersion::new(seed.manifest.catalog_version)?,
        locales,
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
    let locale_set = seed
        .manifest
        .locales
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if locale_set.is_empty() || locale_set.len() != seed.manifest.locales.len() {
        bail!("official Seed locale inventory must be unique and non-empty");
    }

    let forbidden = Regex::new(
        r"(?i)</?[A-Za-z][^>]*>|javascript\s*:|\$\{|=>|\bfunction\s*\(|on[A-Za-z]+\s*=|!?\[[^\]]*\]\([^)]+\)",
    )?;
    let named_placeholder = Regex::new(r"\{([A-Za-z_][A-Za-z0-9_.-]*)\}")?;
    let mut identities = BTreeSet::new();
    for message in &seed.messages {
        if !identities.insert(message.key.as_str()) {
            bail!("official Seed contains a duplicate global key");
        }
        validate_plain_text(&message.key, &forbidden, &named_placeholder)?;
        if message
            .translations
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != locale_set
        {
            bail!("official Seed message locales do not match the manifest");
        }
        let key_placeholders = placeholders(&message.key, &named_placeholder);
        for translation in message.translations.values() {
            validate_plain_text(translation, &forbidden, &named_placeholder)?;
            if placeholders(translation, &named_placeholder) != key_placeholders {
                bail!("official Seed translation placeholder set does not match its key");
            }
        }
    }
    if identities.is_empty() {
        bail!("official Seed message inventory must be non-empty");
    }

    let mut paths = BTreeSet::new();
    let mut groups = BTreeMap::<&str, BTreeMap<&str, BTreeSet<&str>>>::new();
    for file in &seed.manifest.files {
        if !paths.insert(&file.path) || !locale_set.contains(file.locale.as_str()) {
            bail!("official Seed file inventory has an unknown locale or duplicate path");
        }
        let (group, file_name) = file
            .path
            .rsplit_once('/')
            .ok_or_else(|| anyhow!("official Seed file path has no source group"))?;
        if file_name != format!("{}.json", file.locale) {
            bail!("official Seed file path locale does not match its locale");
        }
        let keys = file
            .keys
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if keys.is_empty()
            || keys.len() != file.keys.len()
            || keys.iter().any(|key| !identities.contains(*key))
        {
            bail!("official Seed file has an unknown, duplicate, or empty key inventory");
        }
        groups
            .entry(group)
            .or_default()
            .insert(file.locale.as_str(), keys);
    }
    if groups.is_empty() {
        bail!("official Seed file inventory must be non-empty");
    }
    for files_by_locale in groups.values() {
        if files_by_locale.keys().copied().collect::<BTreeSet<_>>() != locale_set {
            bail!("official Seed source group does not cover every locale");
        }
        let mut key_sets = files_by_locale.values();
        let expected = key_sets
            .next()
            .ok_or_else(|| anyhow!("official Seed source group has no files"))?;
        if key_sets.any(|keys| keys != expected) {
            bail!("official Seed source group locale keys do not match");
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
    let messages = seed
        .messages
        .iter()
        .map(|message| (message.key.as_str(), message))
        .collect::<BTreeMap<_, _>>();
    for file in &seed.manifest.files {
        let document = Value::Object(
            file.keys
                .iter()
                .map(|key| {
                    messages
                        .get(key.as_str())
                        .and_then(|message| message.translations.get(&file.locale))
                        .cloned()
                        .map(|translation| (key.clone(), Value::String(translation)))
                        .ok_or_else(|| anyhow!("official Seed file is missing a translation"))
                })
                .collect::<Result<serde_json::Map<_, _>>>()?,
        );
        // P1 freezes file.keys in the publisher's locale-aware canonical order. Preserve that
        // order because Rust lexical ordering differs for punctuation and case.
        if digest_canonical_json(&document)? != file.sha256 {
            bail!("official Seed file digest mismatch");
        }
    }
    let semantic = json!({
        "catalog_version": seed.manifest.catalog_version,
        "source_locale": seed.manifest.source_locale,
        "locales": seed.manifest.locales,
        "files": seed.manifest.files,
        "messages": seed.messages,
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
