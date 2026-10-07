use super::*;

#[test]
fn curated_collectors_are_client_only_and_localized_without_device_status() {
    for (locale, readme) in [("en_US", "README.en.md"), ("zh_Hans", "README.md")] {
        let collectors =
            application_collector_catalog(&domain::CatalogLocale::new(locale).unwrap());
        assert_eq!(collectors.len(), 1);
        let value = serde_json::to_value(&collectors[0]).unwrap();
        assert_eq!(value["collector_code"], "codex-logs-collector");
        assert_eq!(value["source_client"], "codex");
        assert_eq!(value["execution_target"], "client");
        assert_eq!(value["version"], "0.1.0");
        assert!(value["documentation_url"]
            .as_str()
            .unwrap()
            .ends_with(readme));
        assert!(value["description"].as_str().unwrap().len() > 0);
        assert!(value.get("installed").is_none());
        assert!(value.get("last_seen_at").is_none());
        let output = ApplicationsOutput::Catalog(ApplicationCatalogResponse {
            collectors,
            types: vec![],
            workflow_triggers: vec![],
            tags: vec![],
        });
        let projection = output.project_for_managed_hook().unwrap();
        assert_eq!(
            projection["0"]["collectors"][0]["collector_code"],
            "codex-logs-collector"
        );
        assert!(
            projection["0"]["collectors"][0]["description"]["byte_count"]
                .as_u64()
                .unwrap()
                > 0
        );
        let schema = ApplicationsOutput::managed_projection_schema().unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        assert!(validator.is_valid(&projection));
        let mut missing_collectors = projection.clone();
        missing_collectors["0"]
            .as_object_mut()
            .unwrap()
            .remove("collectors");
        assert!(!validator.is_valid(&missing_collectors));
    }
}
