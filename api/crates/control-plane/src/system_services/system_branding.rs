use control_plane_contracts::{
    ports::{SystemBranding, SystemBrandingRepository},
    ControlPlaneContractError,
};

pub struct SystemBrandingService<R>(R);

impl<R: SystemBrandingRepository> SystemBrandingService<R> {
    pub fn new(repository: R) -> Self {
        Self(repository)
    }

    pub async fn read(&self) -> anyhow::Result<SystemBranding> {
        self.0.get_system_branding().await
    }

    pub async fn save(&self, value: SystemBranding) -> anyhow::Result<SystemBranding> {
        if value.system_name.trim().is_empty() || value.system_name.chars().count() > 100 {
            return Err(ControlPlaneContractError::InvalidInput("invalid_system_name").into());
        }
        if value.system_description.chars().count() > 500 {
            return Err(
                ControlPlaneContractError::InvalidInput("invalid_system_description").into(),
            );
        }
        // Logo uploads travel with the form as a data URL; the default stays a static asset.
        if value.logo_url != "/icon.svg"
            && ![
                "data:image/png;base64,",
                "data:image/jpeg;base64,",
                "data:image/svg+xml;base64,",
            ]
            .iter()
            .any(|prefix| value.logo_url.starts_with(prefix))
        {
            return Err(ControlPlaneContractError::InvalidInput("invalid_system_logo").into());
        }
        if value.logo_url.len() > 750_000 {
            return Err(ControlPlaneContractError::InvalidInput("system_logo_too_large").into());
        }
        // Filing HTML is administrator-authored display content and is stored verbatim.
        self.0.save_system_branding(&value).await?;
        Ok(value)
    }
}
