use super::super::settings::Settings;
use super::*;

const LEGACY_NATIVE_ENV: &str = "# keep my comment\nAPI_ENV=production\nAPI_SERVER_ADDR=127.0.0.1:17800\nAPI_DATABASE_URL=postgres://postgres:__GENERATE__@127.0.0.1:45432/1flowbase\nAPI_PROVIDER_SECRET_MASTER_KEY=__GENERATE__\nFLOWBASE_DATA_DIR=custom data\nBOOTSTRAP_ROOT_ACCOUNT=custom-root\nBOOTSTRAP_ROOT_EMAIL=custom@example.com\nBOOTSTRAP_ROOT_PASSWORD='custom # password'\nBOOTSTRAP_WORKSPACE_NAME=custom workspace\nAPI_WORKSPACE_ROOT=api\nAPI_MODEL_PRICING_BOOTSTRAP_ROOT=api/resources/model-pricing\nAPI_APPLICATION_TEMPLATE_ROOT=api/resources/application-templates\nAPI_POSTGRES_PG_DUMP_PATH=postgres/bin/pg_dump\nAPI_POSTGRES_PG_RESTORE_PATH=postgres/bin/pg_restore\nAPI_BUSINESS_FILE_LOCAL_ROOT=custom data/api/storage\nAPI_PROVIDER_INSTALL_ROOT=custom data/api/plugins\nAPI_HOST_EXTENSION_DROPIN_ROOT=custom data/api/dropins\nAPI_MCP_TEMPLATE_LIBRARY_ROOT=custom data/api/mcp\nAPI_SYSTEM_BACKUP_REPOSITORY_ROOT=custom data/backups\n";

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
        let text = LEGACY_NATIVE_ENV;
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

#[test]
fn native_settings_apply_local_terminal_compatibility_default() {
    const CHILD_ROOT: &str = "FLOWBASE_TEST_NATIVE_TERMINAL_CHILD_ROOT";
    const FLAG: &str = "API_LOCAL_TERMINAL_DISABLED";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        let root = std::path::PathBuf::from(root);
        let settings = Settings::inspect(&root).unwrap();
        assert_eq!(settings.values["API_ENV"], "production");
        assert!(!settings.values.contains_key(FLAG));
        settings.apply(&root).unwrap();
        assert_eq!(std::env::var(FLAG).unwrap(), "false");
        assert_eq!(
            std::fs::read_to_string(root.join(".env")).unwrap(),
            LEGACY_NATIVE_ENV
        );

        std::fs::write(
            root.join(".env"),
            format!("{LEGACY_NATIVE_ENV}{FLAG}=true\n"),
        )
        .unwrap();
        Settings::inspect(&root).unwrap().apply(&root).unwrap();
        assert_eq!(std::env::var(FLAG).unwrap(), "true");
        return;
    }

    // Keep Settings::apply's process-wide environment changes out of other tests.
    let directory = std::env::temp_dir().join(format!("native-terminal-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join(".env"), LEGACY_NATIVE_ENV).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            concat!(
                module_path!(),
                "::native_settings_apply_local_terminal_compatibility_default"
            )
            .split_once("::")
            .unwrap()
            .1,
        ])
        .env(CHILD_ROOT, &directory)
        .env(FLAG, "true")
        .output()
        .unwrap();
    std::fs::remove_dir_all(directory).unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
