use crate::PgControlPlaneStore;
use async_trait::async_trait;
use control_plane_contracts::ports::{SystemBranding, SystemBrandingRepository};
use sqlx::Row;

#[async_trait]
impl SystemBrandingRepository for PgControlPlaneStore {
    async fn get_system_branding(&self) -> anyhow::Result<SystemBranding> {
        let row = sqlx::query("SELECT logo_url, system_name, login_title, system_description, filing_html FROM system_branding WHERE singleton = TRUE")
            .fetch_optional(self.pool()).await?;
        row.map(|row| {
            Ok(SystemBranding {
                logo_url: row.try_get("logo_url")?,
                system_name: row.try_get("system_name")?,
                login_title: row.try_get("login_title")?,
                system_description: row.try_get("system_description")?,
                filing_html: row.try_get("filing_html")?,
            })
        })
        .unwrap_or_else(|| Ok(SystemBranding::default()))
    }

    async fn save_system_branding(&self, value: &SystemBranding) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO system_branding (singleton, logo_url, system_name, login_title, system_description, filing_html) VALUES (TRUE, $1, $2, $3, $4, $5) ON CONFLICT (singleton) DO UPDATE SET logo_url = EXCLUDED.logo_url, system_name = EXCLUDED.system_name, login_title = EXCLUDED.login_title, system_description = EXCLUDED.system_description, filing_html = EXCLUDED.filing_html")
            .bind(&value.logo_url).bind(&value.system_name).bind(&value.login_title)
            .bind(&value.system_description).bind(&value.filing_html)
            .execute(self.pool()).await?;
        Ok(())
    }
}
