use super::super::settings::Settings;
use super::*;

#[test]
fn dotenv_reads_literals_quotes_comments_and_rejects_ambiguous_keys() {
    let values = parse("\u{feff}# heading\r\nexport NAME='value # literal'\r\nPORT = 7800 # comment\nJSON={\"key\":\"a\\nb\"}\nESCAPED=\"a\\nb\"\nLITERAL=$(no-shell)\n").unwrap();
    assert_eq!(values["NAME"], "value # literal");
    assert_eq!(values["PORT"], "7800");
    assert_eq!(values["JSON"], r#"{"key":"a\nb"}"#);
    assert_eq!(values["ESCAPED"], "a\nb");
    assert_eq!(values["LITERAL"], "$(no-shell)");
    for invalid in [
        "A=1\nA=2",
        "bad-key=x",
        "A='unclosed",
        "A=\"quoted\"junk",
        "not an assignment",
    ] {
        assert!(parse(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn user_configuration_wins_and_generated_secrets_persist_in_env_only() {
    let directory = std::env::temp_dir().join(format!("native-env-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir(&directory).unwrap();
    {
        let text = "# keep my comment\nAPI_ENV=production\nAPI_SERVER_ADDR=127.0.0.1:17800\nAPI_DATABASE_URL=postgres://postgres:__GENERATE__@127.0.0.1:45432/1flowbase\nAPI_PROVIDER_SECRET_MASTER_KEY=__GENERATE__\nFLOWBASE_DATA_DIR=custom data\nBOOTSTRAP_ROOT_ACCOUNT=custom-root\nBOOTSTRAP_ROOT_EMAIL=custom@example.com\nBOOTSTRAP_ROOT_PASSWORD='custom # password'\nBOOTSTRAP_WORKSPACE_NAME=custom workspace\nAPI_WORKSPACE_ROOT=api\nAPI_MODEL_PRICING_BOOTSTRAP_ROOT=api/resources/model-pricing\nAPI_APPLICATION_TEMPLATE_ROOT=api/resources/application-templates\nAPI_POSTGRES_PG_DUMP_PATH=postgres/bin/pg_dump\nAPI_POSTGRES_PG_RESTORE_PATH=postgres/bin/pg_restore\nAPI_BUSINESS_FILE_LOCAL_ROOT=custom data/api/storage\nAPI_PROVIDER_INSTALL_ROOT=custom data/api/plugins\nAPI_HOST_EXTENSION_DROPIN_ROOT=custom data/api/dropins\nAPI_MCP_TEMPLATE_LIBRARY_ROOT=custom data/api/mcp\nAPI_SYSTEM_BACKUP_REPOSITORY_ROOT=custom data/backups\n";
        std::fs::write(directory.join(".env"), text).unwrap();
        let first = Settings::load(&directory).unwrap();
        assert_eq!(first.address.port(), 17800);
        assert_eq!(first.postgres_port, 45432);
        assert_eq!(first.values["BOOTSTRAP_ROOT_PASSWORD"], "custom # password");
        assert_eq!(first.values["BOOTSTRAP_ROOT_ACCOUNT"], "custom-root");
        assert_eq!(first.data, directory.join("custom data"));
        assert_ne!(first.postgres_password, "__GENERATE__");
        let saved = std::fs::read(directory.join(".env")).unwrap();
        let second = Settings::load(&directory).unwrap();
        assert_eq!(second.postgres_password, first.postgres_password);
        assert_eq!(
            second.values["API_PROVIDER_SECRET_MASTER_KEY"],
            first.values["API_PROVIDER_SECRET_MASTER_KEY"]
        );
        assert_eq!(std::fs::read(directory.join(".env")).unwrap(), saved);
        assert!(String::from_utf8(saved)
            .unwrap()
            .starts_with("# keep my comment"));
        assert!(!directory.join("data/settings.json").exists());
        std::fs::create_dir_all(first.data.join("postgres")).unwrap();
        std::fs::write(directory.join(".env"), text).unwrap();
        assert!(
            Settings::load(&directory).is_err(),
            "existing data must never receive new secrets"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}
