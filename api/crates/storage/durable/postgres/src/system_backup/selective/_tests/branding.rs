use super::*;
use control_plane_contracts::ports::{SystemBranding, SystemBrandingRepository};

#[tokio::test]
async fn selective_branding_configuration_roundtrips_the_complete_singleton() {
    let (db, _) = fixture().await;
    let store = crate::PgControlPlaneStore::new(db.clone());
    let repo = PgSelectiveBackupRepository::new(db);
    let original = SystemBranding {
        logo_url: "/brand.svg".into(),
        system_name: "Configured system".into(),
        hide_login_tagline: true,
        system_description: "Installation description".into(),
        filing_html: "<style>a{color:red}</style><a href=\"https://example.com\">备案</a>".into(),
    };
    store.save_system_branding(&original).await.unwrap();
    let categories = repo.catalog().await.unwrap();
    let branding = categories
        .iter()
        .find(|category| category.feature_id == "system.branding")
        .unwrap();
    assert_eq!(branding.label_key, "system_settings.title");
    assert_eq!(branding.structure_tables, ["system_branding"]);
    assert!(branding.data_tables.is_empty());

    let bytes = capture(&repo, select("branding", true, false)).await;
    let changed = SystemBranding::default();
    store.save_system_branding(&changed).await.unwrap();
    let pending = repo
        .prepare_restore(reader(bytes.clone()), "source", "target", true)
        .await
        .unwrap();
    pending.rollback().await.unwrap();
    assert_eq!(store.get_system_branding().await.unwrap(), changed);

    repo.restore(reader(bytes), "source", "target", true)
        .await
        .unwrap();
    assert_eq!(store.get_system_branding().await.unwrap(), original);
}
