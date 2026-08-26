use std::collections::BTreeMap;

use domain::{
    CatalogDigest, CatalogLocale, CatalogMessageIdentity, CatalogSeedFile, CatalogVersion,
    I18nCatalogInvariantError, OfficialCatalogMessage,
};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    CatalogResolutionRepository, I18nCatalogManagementRepository, I18nCatalogRepository,
    OfficialI18nCatalogReleaseDescriptor, OfficialI18nCatalogSourcePort,
    RuntimeI18nCatalogRepository, VerifiedOfficialCatalogSeed,
};

fn digest(character: char) -> CatalogDigest {
    CatalogDigest::new(format!("sha256:{}", character.to_string().repeat(64)))
        .expect("valid digest")
}

fn official_message(target: &CatalogLocale) -> OfficialCatalogMessage {
    let mut translations = BTreeMap::new();
    translations.insert(CatalogLocale::source(), "Save {name}".to_string());
    translations.insert(target.clone(), "保存 {name}".to_string());
    OfficialCatalogMessage::new(
        CatalogMessageIdentity::new("Save {name}").expect("valid message identity"),
        translations,
    )
    .expect("valid official message")
}

#[test]
fn verified_seed_keeps_domain_validation_and_workspace_binding() {
    adapter_traits_remain_canonical::<
        dyn OfficialI18nCatalogSourcePort,
        dyn CatalogResolutionRepository,
        dyn RuntimeI18nCatalogRepository,
        dyn I18nCatalogManagementRepository,
        dyn I18nCatalogRepository,
    >();

    let target = CatalogLocale::new("zh_Hans").expect("valid target locale");
    let version = CatalogVersion::new("2026.08.26").expect("valid version");
    let semantic = digest('a');
    let seed = VerifiedOfficialCatalogSeed::new(
        Uuid::now_v7(),
        version.clone(),
        vec![CatalogLocale::source(), target.clone()],
        vec![
            CatalogSeedFile::new(target.clone(), "common/zh_Hans.json", digest('b'))
                .expect("valid seed file"),
        ],
        OffsetDateTime::UNIX_EPOCH,
        semantic.clone(),
        vec![official_message(&target)],
    )
    .expect("valid seed");

    let workspace_id = Uuid::now_v7();
    let release = seed
        .bind_to_workspace(workspace_id)
        .expect("valid workspace binding");
    assert_eq!(release.workspace_id(), workspace_id);
    assert_eq!(seed.catalog_version(), &version);
    assert_eq!(seed.semantic_sha256(), &semantic);

    let descriptor = OfficialI18nCatalogReleaseDescriptor {
        catalog_version: version,
        semantic_sha256: semantic,
        seed_sha256: digest('c'),
    };
    assert_eq!(descriptor.clone(), descriptor);
}

#[test]
fn verified_seed_rejects_catalogs_without_the_source_locale() {
    let target = CatalogLocale::new("zh_Hans").expect("valid target locale");
    let error = VerifiedOfficialCatalogSeed::new(
        Uuid::now_v7(),
        CatalogVersion::new("2026.08.26").expect("valid version"),
        vec![target.clone()],
        vec![
            CatalogSeedFile::new(target.clone(), "common/zh_Hans.json", digest('b'))
                .expect("valid seed file"),
        ],
        OffsetDateTime::UNIX_EPOCH,
        digest('a'),
        vec![official_message(&target)],
    )
    .expect_err("domain invariant gate must reject a missing source locale");

    assert_eq!(error, I18nCatalogInvariantError::MissingSourceLocale);
}

fn adapter_traits_remain_canonical<Source, Resolution, Runtime, Management, Repository>()
where
    Source: OfficialI18nCatalogSourcePort + ?Sized,
    Resolution: CatalogResolutionRepository + ?Sized,
    Runtime: RuntimeI18nCatalogRepository + ?Sized,
    Management: I18nCatalogManagementRepository + ?Sized,
    Repository: I18nCatalogRepository + ?Sized,
{
}
