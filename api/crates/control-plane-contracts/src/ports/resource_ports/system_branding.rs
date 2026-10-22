use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Installation-wide public display metadata. Contains no credentials or workspace data.
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

#[async_trait]
pub trait SystemBrandingRepository: Send + Sync {
    async fn get_system_branding(&self) -> anyhow::Result<SystemBranding>;
    async fn save_system_branding(&self, value: &SystemBranding) -> anyhow::Result<()>;
}
