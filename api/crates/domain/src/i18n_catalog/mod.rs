use std::collections::{BTreeMap, BTreeSet};

use time::OffsetDateTime;
use uuid::Uuid;

pub const I18N_CATALOG_SEED_SCHEMA_VERSION: &str = "1flowbase.i18n-catalog-seed/v2";
pub const I18N_CATALOG_SOURCE_LOCALE: &str = "en_US";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum I18nCatalogInvariantError {
    EmptyCatalogVersion,
    InvalidDigest,
    InvalidLocale,
    EmptyMessageKey,
    InvalidMessageKey,
    MissingSourceLocale,
    MissingSourceTranslation,
    DuplicateLocale,
    DuplicateMessageIdentity,
    EmptySeedFilePath,
    UnknownFileLocale,
    UnknownTranslationLocale,
    InvalidRevision,
}

impl std::fmt::Display for I18nCatalogInvariantError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid i18n catalog invariant: {self:?}")
    }
}

impl std::error::Error for I18nCatalogInvariantError {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CatalogVersion(String);

impl CatalogVersion {
    pub fn new(value: impl Into<String>) -> Result<Self, I18nCatalogInvariantError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(I18nCatalogInvariantError::EmptyCatalogVersion);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CatalogDigest(String);

impl CatalogDigest {
    pub fn new(value: impl Into<String>) -> Result<Self, I18nCatalogInvariantError> {
        let value = value.into();
        let hexadecimal = value.strip_prefix("sha256:");
        if hexadecimal.is_none_or(|digest| {
            digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }) {
            return Err(I18nCatalogInvariantError::InvalidDigest);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CatalogLocale(String);

impl CatalogLocale {
    pub fn new(value: impl Into<String>) -> Result<Self, I18nCatalogInvariantError> {
        let value = value.into();
        let mut parts = value.split('_');
        let language = parts.next().unwrap_or_default();
        let suffix = parts.next();
        let language_is_valid = (2..=3).contains(&language.len())
            && language.bytes().all(|byte| byte.is_ascii_lowercase());
        let suffix_is_valid = suffix.is_none_or(|suffix| {
            let mut bytes = suffix.bytes();
            bytes.next().is_some_and(|byte| byte.is_ascii_uppercase())
                && (1..=7).contains(&bytes.len())
                && bytes.all(|byte| byte.is_ascii_alphabetic())
        });
        if !language_is_valid || !suffix_is_valid || parts.next().is_some() {
            return Err(I18nCatalogInvariantError::InvalidLocale);
        }
        Ok(Self(value))
    }

    pub fn source() -> Self {
        Self(I18N_CATALOG_SOURCE_LOCALE.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_source(&self) -> bool {
        self.0 == I18N_CATALOG_SOURCE_LOCALE
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CatalogMessageIdentity {
    key: String,
}

impl CatalogMessageIdentity {
    pub fn new(key: impl Into<String>) -> Result<Self, I18nCatalogInvariantError> {
        let key = key.into();
        if key.trim().is_empty() {
            return Err(I18nCatalogInvariantError::EmptyMessageKey);
        }
        if key.trim() != key
            || !key.is_ascii()
            || key.chars().any(char::is_control)
            || !key.bytes().any(|byte| byte.is_ascii_alphabetic())
            || contains_unsafe_message_key_content(&key)
            || looks_like_variable_key(&key)
        {
            return Err(I18nCatalogInvariantError::InvalidMessageKey);
        }
        Ok(Self { key })
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

fn contains_unsafe_message_key_content(key: &str) -> bool {
    let lowercase = key.to_ascii_lowercase();
    key.contains('<')
        || key.contains('>')
        || key.contains("${")
        || key.contains("=>")
        || lowercase.contains("javascript:")
        || lowercase.contains("function(")
}

fn looks_like_variable_key(key: &str) -> bool {
    if key.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return false;
    }
    let identifier_part = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    };
    (key.contains('.') && key.split('.').all(identifier_part))
        || (key.contains('_') && key.split('_').all(identifier_part))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSeedFile {
    locale: CatalogLocale,
    path: String,
    sha256: CatalogDigest,
}

impl CatalogSeedFile {
    pub fn new(
        locale: CatalogLocale,
        path: impl Into<String>,
        sha256: CatalogDigest,
    ) -> Result<Self, I18nCatalogInvariantError> {
        let path = path.into();
        if path.trim().is_empty() {
            return Err(I18nCatalogInvariantError::EmptySeedFilePath);
        }
        Ok(Self {
            locale,
            path,
            sha256,
        })
    }

    pub fn locale(&self) -> &CatalogLocale {
        &self.locale
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn sha256(&self) -> &CatalogDigest {
        &self.sha256
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialCatalogMessage {
    identity: CatalogMessageIdentity,
    translations: BTreeMap<CatalogLocale, String>,
}

impl OfficialCatalogMessage {
    pub fn new(
        identity: CatalogMessageIdentity,
        translations: BTreeMap<CatalogLocale, String>,
    ) -> Result<Self, I18nCatalogInvariantError> {
        if !translations.keys().any(CatalogLocale::is_source) {
            return Err(I18nCatalogInvariantError::MissingSourceTranslation);
        }
        Ok(Self {
            identity,
            translations,
        })
    }

    pub fn identity(&self) -> &CatalogMessageIdentity {
        &self.identity
    }
    pub fn translations(&self) -> &BTreeMap<CatalogLocale, String> {
        &self.translations
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCatalogRelease {
    id: Uuid,
    workspace_id: Uuid,
    catalog_version: CatalogVersion,
    locales: Vec<CatalogLocale>,
    files: Vec<CatalogSeedFile>,
    generated_at: OffsetDateTime,
    semantic_sha256: CatalogDigest,
    messages: Vec<OfficialCatalogMessage>,
}

impl VerifiedCatalogRelease {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Uuid,
        workspace_id: Uuid,
        catalog_version: CatalogVersion,
        locales: Vec<CatalogLocale>,
        files: Vec<CatalogSeedFile>,
        generated_at: OffsetDateTime,
        semantic_sha256: CatalogDigest,
        messages: Vec<OfficialCatalogMessage>,
    ) -> Result<Self, I18nCatalogInvariantError> {
        if locales.iter().collect::<BTreeSet<_>>().len() != locales.len() {
            return Err(I18nCatalogInvariantError::DuplicateLocale);
        }
        if !locales.iter().any(CatalogLocale::is_source) {
            return Err(I18nCatalogInvariantError::MissingSourceLocale);
        }
        let locale_set = locales.iter().collect::<BTreeSet<_>>();
        if files.iter().any(|file| !locale_set.contains(file.locale())) {
            return Err(I18nCatalogInvariantError::UnknownFileLocale);
        }
        if messages
            .iter()
            .map(OfficialCatalogMessage::identity)
            .collect::<BTreeSet<_>>()
            .len()
            != messages.len()
        {
            return Err(I18nCatalogInvariantError::DuplicateMessageIdentity);
        }
        if messages
            .iter()
            .flat_map(OfficialCatalogMessage::translations)
            .any(|(locale, _)| !locale_set.contains(locale))
        {
            return Err(I18nCatalogInvariantError::UnknownTranslationLocale);
        }
        Ok(Self {
            id,
            workspace_id,
            catalog_version,
            locales,
            files,
            generated_at,
            semantic_sha256,
            messages,
        })
    }

    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }
    pub fn catalog_version(&self) -> &CatalogVersion {
        &self.catalog_version
    }
    pub fn locales(&self) -> &[CatalogLocale] {
        &self.locales
    }
    pub fn files(&self) -> &[CatalogSeedFile] {
        &self.files
    }
    pub fn generated_at(&self) -> OffsetDateTime {
        self.generated_at
    }
    pub fn semantic_sha256(&self) -> &CatalogDigest {
        &self.semantic_sha256
    }
    pub fn messages(&self) -> &[OfficialCatalogMessage] {
        &self.messages
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkspaceCatalogRevision(i64);

impl WorkspaceCatalogRevision {
    pub fn initial() -> Self {
        Self(0)
    }
    pub fn new(value: i64) -> Result<Self, I18nCatalogInvariantError> {
        if value < 0 {
            return Err(I18nCatalogInvariantError::InvalidRevision);
        }
        Ok(Self(value))
    }
    pub fn value(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceCatalogState {
    workspace_id: Uuid,
    active_release_id: Option<Uuid>,
    revision: WorkspaceCatalogRevision,
}

impl WorkspaceCatalogState {
    pub fn restored(
        workspace_id: Uuid,
        active_release_id: Option<Uuid>,
        revision: WorkspaceCatalogRevision,
    ) -> Self {
        Self {
            workspace_id,
            active_release_id,
            revision,
        }
    }
    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }
    pub fn active_release_id(&self) -> Option<Uuid> {
        self.active_release_id
    }
    pub fn revision(&self) -> WorkspaceCatalogRevision {
        self.revision
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogTranslation {
    identity: CatalogMessageIdentity,
    locale: CatalogLocale,
    translation: String,
}

impl CatalogTranslation {
    pub fn new(
        identity: CatalogMessageIdentity,
        locale: CatalogLocale,
        translation: impl Into<String>,
    ) -> Result<Self, I18nCatalogInvariantError> {
        Ok(Self {
            identity,
            locale,
            translation: translation.into(),
        })
    }
    pub fn identity(&self) -> &CatalogMessageIdentity {
        &self.identity
    }
    pub fn locale(&self) -> &CatalogLocale {
        &self.locale
    }
    pub fn translation(&self) -> &str {
        &self.translation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveOfficialCatalogMessage {
    release_id: Uuid,
    message: OfficialCatalogMessage,
}

impl ActiveOfficialCatalogMessage {
    pub fn restored(release_id: Uuid, message: OfficialCatalogMessage) -> Self {
        Self {
            release_id,
            message,
        }
    }
    pub fn release_id(&self) -> Uuid {
        self.release_id
    }
    pub fn message(&self) -> &OfficialCatalogMessage {
        &self.message
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObsoleteCatalogMessage {
    identity: CatalogMessageIdentity,
    obsolete_since_release_id: Uuid,
}

impl ObsoleteCatalogMessage {
    pub fn restored(identity: CatalogMessageIdentity, obsolete_since_release_id: Uuid) -> Self {
        Self {
            identity,
            obsolete_since_release_id,
        }
    }
    pub fn identity(&self) -> &CatalogMessageIdentity {
        &self.identity
    }
    pub fn obsolete_since_release_id(&self) -> Uuid {
        self.obsolete_since_release_id
    }
}
