use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Installation-wide public display metadata persisted in `system_branding`.
/// Contains no credentials or workspace data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SystemBranding {
    pub logo_url: String,
    pub system_name: String,
    pub hide_login_tagline: bool,
    pub system_description: String,
    pub filing_html: String,
}

impl Default for SystemBranding {
    fn default() -> Self {
        Self {
            logo_url: "/icon.svg".into(),
            system_name: "1flowbase".into(),
            hide_login_tagline: false,
            system_description: "对话即是壁垒，AI应用原生底座".into(),
            filing_html: String::new(),
        }
    }
}

/// Static JSON white-label configuration owned by `@taichuy/commercial-license`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommercialBrandingConfig {
    #[serde(default)]
    pub hide_help_menu: bool,
    #[serde(default)]
    pub hide_powered_by: bool,
    #[serde(default)]
    pub page_title: String,
    #[serde(default)]
    pub login_hero_title: String,
    #[serde(default)]
    pub login_hero_tagline: String,
}

/// Effective read-only public system display projection served by `GET /api/public/system-metadata`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicSystemMetadata {
    pub logo_url: String,
    pub system_name: String,
    pub hide_login_tagline: bool,
    pub system_description: String,
    pub filing_html: String,
    pub page_title: String,
    pub login_hero_title: String,
    pub login_hero_tagline: Option<String>,
    pub hide_powered_by: bool,
    pub hide_help_menu: bool,
}

impl PublicSystemMetadata {
    pub fn compose(base: SystemBranding, commercial: Option<&CommercialBrandingConfig>) -> Self {
        let (page_title, login_hero_title, login_hero_tagline, hide_powered_by, hide_help_menu) =
            match commercial {
                Some(config) => {
                    let page_title = {
                        let trimmed = config.page_title.trim();
                        if trimmed.is_empty() {
                            base.system_name.clone()
                        } else {
                            trimmed.to_owned()
                        }
                    };
                    let login_hero_title = {
                        let trimmed = config.login_hero_title.trim();
                        if trimmed.is_empty() {
                            "1flowbase".to_owned()
                        } else {
                            trimmed.to_owned()
                        }
                    };
                    let login_hero_tagline = {
                        let trimmed = config.login_hero_tagline.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed.to_owned())
                        }
                    };
                    (
                        page_title,
                        login_hero_title,
                        login_hero_tagline,
                        config.hide_powered_by,
                        config.hide_help_menu,
                    )
                }
                None => (
                    base.system_name.clone(),
                    "1flowbase".to_owned(),
                    None,
                    false,
                    false,
                ),
            };

        Self {
            logo_url: base.logo_url,
            system_name: base.system_name,
            hide_login_tagline: base.hide_login_tagline,
            system_description: base.system_description,
            filing_html: base.filing_html,
            page_title,
            login_hero_title,
            login_hero_tagline,
            hide_powered_by,
            hide_help_menu,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CommercialBrandingConfig, PublicSystemMetadata, SystemBranding};

    #[test]
    fn compose_without_commercial_plugin_preserves_standard_defaults() {
        let base = SystemBranding {
            logo_url: "/icon.svg".into(),
            system_name: "Acme Workspace".into(),
            hide_login_tagline: true,
            system_description: "Desc".into(),
            filing_html: "<span>ICP</span>".into(),
        };
        let composed = PublicSystemMetadata::compose(base, None);
        assert_eq!(composed.system_name, "Acme Workspace");
        assert_eq!(composed.page_title, "Acme Workspace");
        assert_eq!(composed.login_hero_title, "1flowbase");
        assert_eq!(composed.login_hero_tagline, None);
        assert!(!composed.hide_powered_by);
        assert!(!composed.hide_help_menu);
        assert!(composed.hide_login_tagline);
        assert_eq!(composed.filing_html, "<span>ICP</span>");
    }

    #[test]
    fn compose_with_commercial_plugin_applies_white_label_orthogonally() {
        let base = SystemBranding {
            logo_url: "/icon.svg".into(),
            system_name: "Console Name".into(),
            hide_login_tagline: false,
            system_description: "Desc".into(),
            filing_html: "<span>ICP</span>".into(),
        };
        let commercial = CommercialBrandingConfig {
            hide_help_menu: true,
            hide_powered_by: true,
            page_title: "  Acme AI  ".into(),
            login_hero_title: "  Acme Engine  ".into(),
            login_hero_tagline: "  Enterprise Native Base  ".into(),
        };
        let composed = PublicSystemMetadata::compose(base, Some(&commercial));
        assert_eq!(composed.system_name, "Console Name");
        assert_eq!(composed.page_title, "Acme AI");
        assert_eq!(composed.login_hero_title, "Acme Engine");
        assert_eq!(
            composed.login_hero_tagline.as_deref(),
            Some("Enterprise Native Base")
        );
        assert!(composed.hide_powered_by);
        assert!(composed.hide_help_menu);
        assert!(!composed.hide_login_tagline);
        assert_eq!(composed.filing_html, "<span>ICP</span>");
    }
}

#[async_trait]
pub trait SystemBrandingRepository: Send + Sync {
    async fn get_system_branding(&self) -> anyhow::Result<SystemBranding>;
    async fn save_system_branding(&self, value: &SystemBranding) -> anyhow::Result<()>;
}
