use super::parse_bundle_archive;

#[test]
fn rejects_non_zip_bundle() {
    assert!(parse_bundle_archive(b"not-a-zip").is_err());
}
