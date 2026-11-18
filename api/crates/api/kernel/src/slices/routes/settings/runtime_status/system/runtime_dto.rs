use runtime_profile::{LocaleResolution, LocaleSource};
use serde::Serialize;
use utoipa::ToSchema;
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LocaleSourceResponse {
    Query,
    ExplicitHeader,
    UserPreferredLocale,
    AcceptLanguage,
    Fallback,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LocaleMetaResponse {
    pub requested_locale: Option<String>,
    pub resolved_locale: String,
    pub source: LocaleSourceResponse,
    pub fallback_locale: String,
    pub supported_locales: Vec<String>,
}

impl From<LocaleResolution> for LocaleMetaResponse {
    fn from(value: LocaleResolution) -> Self {
        Self {
            requested_locale: value.requested_locale,
            resolved_locale: value.resolved_locale,
            source: value.source.into(),
            fallback_locale: value.fallback_locale,
            supported_locales: value.supported_locales,
        }
    }
}

impl From<LocaleSource> for LocaleSourceResponse {
    fn from(value: LocaleSource) -> Self {
        match value {
            LocaleSource::Query => Self::Query,
            LocaleSource::ExplicitHeader => Self::ExplicitHeader,
            LocaleSource::UserPreferredLocale => Self::UserPreferredLocale,
            LocaleSource::AcceptLanguage => Self::AcceptLanguage,
            LocaleSource::Fallback => Self::Fallback,
        }
    }
}
