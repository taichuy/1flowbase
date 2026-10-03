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
