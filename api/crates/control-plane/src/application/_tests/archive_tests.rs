use serde_json::json;

use crate::application::archive::{
    normalized_application_archive_digest, ApplicationArchiveApplication, ApplicationArchiveEntry,
};

fn archive_entry() -> ApplicationArchiveEntry {
    ApplicationArchiveEntry {
        template_id: "018f0000-0000-7000-8000-000000000001".into(),
        release_version: 1,
        exported_from_system_version: "0.2.0".into(),
        exported_at: "2026-08-02T00:00:00Z".into(),
        application: ApplicationArchiveApplication {
            application_type: "agent_flow".into(),
            workflow_trigger_type: None,
            name: "Release fixture".into(),
            description: "stable digest".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        },
        flow_document: json!({"graph": {"nodes": []}}),
        dependencies: Vec::new(),
        workflow_trigger_config: None,
    }
}

#[test]
fn ac_001_archive_digest_ignores_release_metadata_but_tracks_content() {
    let original = archive_entry();
    let mut metadata_changed = original.clone();
    metadata_changed.release_version = 99;
    metadata_changed.exported_at = "2027-01-01T00:00:00Z".into();
    metadata_changed.exported_from_system_version = "9.0.0".into();

    assert_eq!(
        normalized_application_archive_digest(&original).unwrap(),
        normalized_application_archive_digest(&metadata_changed).unwrap()
    );

    let mut reordered = original.clone();
    reordered.flow_document =
        serde_json::from_str(r#"{"graph":{"nodes":[]},"metadata":{"z":1,"a":2}}"#).unwrap();
    let mut same_content_reordered = original.clone();
    same_content_reordered.flow_document =
        serde_json::from_str(r#"{"metadata":{"a":2,"z":1},"graph":{"nodes":[]}}"#).unwrap();
    assert_eq!(
        normalized_application_archive_digest(&reordered).unwrap(),
        normalized_application_archive_digest(&same_content_reordered).unwrap()
    );

    let mut content_changed = original.clone();
    content_changed.flow_document = json!({"graph": {"nodes": [{"id": "new-node"}]}});
    assert_ne!(
        normalized_application_archive_digest(&original).unwrap(),
        normalized_application_archive_digest(&content_changed).unwrap()
    );
}
