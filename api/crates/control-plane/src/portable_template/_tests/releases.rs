use super::snapshot;
use crate::portable_template::*;
use control_plane_contracts::ports::ApplicationTemplateReleaseRecord;

fn release(version: u64) -> PortableTemplateRelease {
    PortableTemplateRelease {
        template_id: "@taichuy/gateway-demo".into(),
        release_version: version,
        name: "Gateway demo".into(),
        description: "Demo".into(),
        exported_from_system_version: "0.4.1".into(),
        exported_at: "2026-10-03T00:00:00Z".into(),
    }
}

#[test]
fn application_template_release_requires_success_before_skipping() {
    let release = release(1);
    assert!(application_template_needs_install(&release, "digest", &[]).unwrap());
    let mut records = vec![ApplicationTemplateReleaseRecord {
        release_version: 1,
        checksum: "digest".into(),
        successful: false,
    }];
    assert!(application_template_needs_install(&release, "digest", &records).unwrap());
    records[0].successful = true;
    assert!(!application_template_needs_install(&release, "digest", &records).unwrap());
    assert!(application_template_needs_install(&release, "changed", &records).is_err());
    assert!(application_template_needs_install(&self::release(2), "next", &records).unwrap());
    records[0].release_version = 2;
    assert!(application_template_needs_install(&release, "digest", &records).is_err());
}

#[test]
fn application_template_checksum_covers_content_and_release_but_not_json_whitespace() {
    let mut package = snapshot();
    package.release = Some(release(1));
    let digest = application_template_checksum(&package).unwrap();
    let roundtrip: PortableTemplatePackage =
        serde_json::from_str(&serde_json::to_string_pretty(&package).unwrap()).unwrap();
    assert_eq!(digest, application_template_checksum(&roundtrip).unwrap());
    package.release.as_mut().unwrap().release_version = 2;
    assert_ne!(digest, application_template_checksum(&package).unwrap());
    package.release.as_mut().unwrap().release_version = 1;
    package.pages.push(super::group(71, None));
    assert_ne!(digest, application_template_checksum(&package).unwrap());
}

#[test]
fn application_template_release_rejects_invalid_versions_and_keeps_unversioned_packages_compatible()
{
    assert!(validate_application_template_release(&release(0)).is_err());
    assert!(validate_application_template_release(&release(u64::MAX)).is_err());
    let package = serde_json::to_value(snapshot()).unwrap();
    assert!(package.get("release").is_none());
    assert!(serde_json::from_value::<PortableTemplatePackage>(package)
        .unwrap()
        .release
        .is_none());
}

#[test]
fn application_template_rejects_previous_successful_release_after_upgrade() {
    let mut records = vec![
        ApplicationTemplateReleaseRecord {
            release_version: 1,
            checksum: "v1-digest".into(),
            successful: true,
        },
        ApplicationTemplateReleaseRecord {
            release_version: 2,
            checksum: "v2-digest".into(),
            successful: true,
        },
    ];
    for _ in 0..2 {
        let error =
            application_template_needs_install(&release(1), "v1-digest", &records).unwrap_err();
        assert!(error
            .to_string()
            .contains("application_template_release_downgrade"));
        assert!(!application_template_needs_install(&release(2), "v2-digest", &records).unwrap());
        records.reverse();
    }
}

#[test]
fn legacy_checksum_excludes_empty_translation_field_and_v2_covers_translations() {
    use sha2::{Digest, Sha256};
    // Frozen pre-i18n serialized contract: empty translations must not add a field
    // or force a v1 package to v2 merely because the new reader parsed it.
    let legacy_wire = r#"{"schema_version":"1flowbase.portable-template/v1","pages":[],"applications":[],"data_models":[],"mcp_bundle":null,"plugins":[]}"#;
    let legacy: PortableTemplatePackage = serde_json::from_str(legacy_wire).unwrap();
    let expected = format!("{:x}", Sha256::digest(legacy_wire.as_bytes()));
    assert_eq!(application_template_checksum(&legacy).unwrap(), expected);
    let mut explicit_empty = serde_json::to_value(&legacy).unwrap();
    explicit_empty["i18n_entries"] = serde_json::json!([]);
    let explicit_empty: PortableTemplatePackage = serde_json::from_value(explicit_empty).unwrap();
    assert_eq!(
        application_template_checksum(&explicit_empty).unwrap(),
        expected
    );

    let mut translated = legacy;
    translated.schema_version = PORTABLE_TEMPLATE_I18N_SCHEMA_VERSION.into();
    translated.release = Some(release(1));
    translated.i18n_entries.push(PortableI18nEntry {
        key: "template.demo.title".into(),
        locale: "en_US".into(),
        translation: "Title".into(),
    });
    assert!(validate_portable_template(&translated).is_empty());
    validate_application_template_release(translated.release.as_ref().unwrap()).unwrap();
    let checksum = application_template_checksum(&translated).unwrap();
    let roundtrip: PortableTemplatePackage =
        serde_json::from_slice(&serde_json::to_vec(&translated).unwrap()).unwrap();
    assert_eq!(application_template_checksum(&roundtrip).unwrap(), checksum);
    translated.i18n_entries[0].translation = "Updated title".into();
    assert_ne!(
        application_template_checksum(&translated).unwrap(),
        checksum
    );
}
