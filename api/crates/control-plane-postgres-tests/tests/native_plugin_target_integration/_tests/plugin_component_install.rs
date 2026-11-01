//! Real v2 package intake -> install service -> durable transaction -> reusable UI records.
use super::*;

fn package(fixture: &Fixture, version: &str, count: usize) -> PathBuf {
    let directory = fixture.root.join(format!("managed-components-{version}"));
    std::fs::create_dir_all(directory.join("bin")).unwrap();
    std::fs::create_dir_all(directory.join("ui")).unwrap();
    let base =
        include_str!("../../../../extension-package-runtime/src/_tests/managed_manifest.yaml");
    let mut manifest = base.replace("0.1.0", version);
    manifest.push_str("\nblock_contributions:\n");
    for index in 0..count {
        // Manifest module identities are package-wide unique; consumers share this host module.
        let code_modules = if index == 0 {
            "\n      - source: react\n        version: '19.0.0'\n        exports: [useState]\n        binding: host\n        assets: []\n        type_declarations: 'export declare function useState(value: string): [string];'"
        } else {
            " []"
        };
        manifest.push_str(&format!(
            r#"  - contribution_code: view-{index}
    title: View {index}
    runtime: native_react
    entry: View{index}
    code_template: |
      import {{ useState }} from 'react';
      export default function View{index}() {{
        const [label] = useState('{version}');
        return <p>{{label}}</p>;
      }}
    code_template_version: {version}
    code_template_language: tsx
    code_modules:{code_modules}
    context_contract:
      primitives: []
      input_schema: {{type: object}}
    permissions: {{network: none, storage: none, secrets: none}}
    ui_capabilities: []
"#
        ));
    }
    std::fs::write(directory.join("manifest.yaml"), manifest).unwrap();
    std::fs::write(directory.join("bin/fixture"), "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::write(directory.join("ui/render.json"), "{}").unwrap();
    directory
}

#[tokio::test]
async fn v2_install_projects_six_reusable_records_and_upgrade_removes_without_duplicates() {
    let fixture = Fixture::new().await;
    let install = |root: PathBuf| InstallPluginCommand {
        actor_user_id: fixture.actor,
        package_root: root.display().to_string(),
    };
    let v1 = package(&fixture, "0.1.0", 6);
    fixture
        .service
        .install_plugin(install(v1.clone()))
        .await
        .unwrap();
    let records = fixture.store.list_ui_component_records().await.unwrap();
    assert_eq!(records.len(), 6);
    assert!(records
        .iter()
        .all(|record| record.origin == domain::UiComponentRecordOrigin::Plugin));
    assert!(records
        .iter()
        .all(|record| record.owner_plugin_id.as_deref() == Some("managed_fixture")));
    assert_studio_copy_compiles(&records);
    fixture.service.install_plugin(install(v1)).await.unwrap();
    assert_eq!(
        fixture
            .store
            .list_ui_component_records()
            .await
            .unwrap()
            .iter()
            .map(|record| record.id)
            .collect::<Vec<_>>(),
        records.iter().map(|record| record.id).collect::<Vec<_>>()
    );
    fixture
        .service
        .install_plugin(install(package(&fixture, "0.2.0", 5)))
        .await
        .unwrap();
    let upgrade = fixture.store.list_ui_component_records().await.unwrap();
    assert_eq!(upgrade.len(), 5);
    for (before, after) in records.iter().zip(&upgrade) {
        assert_eq!(before.id, after.id);
        assert_eq!(before.component_code, after.component_code);
        assert_ne!(before.owner_installation_id, after.owner_installation_id);
        assert_eq!(after.version, "0.2.0");
        assert!(after.source_code.contains("0.2.0"));
    }
}

// Use the actual Host compiler rather than source-text regex checks or a second TSX compiler.
// This requires the assembly's installed web workspace dependencies; no service is started.
fn assert_studio_copy_compiles(records: &[domain::UiComponentRecord]) {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/native_plugin_target_integration/_tests/compile_studio_copy.cjs");
    let sources: Vec<_> = records
        .iter()
        .map(|record| {
            assert!(record.import_code.starts_with("// Plugin component: "));
            assert_eq!(record.source_code.matches("import { useState }").count(), 1);
            format!("{}\n\n{}", record.import_code, record.source_code)
        })
        .collect();
    let mut child = Command::new("node")
        .arg(script)
        .arg(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Node and installed web dependencies are required for the Host compiler fixture");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&sources).unwrap())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "actual Studio compile failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let compiled: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(compiled["compiled"], records.len());
    assert_eq!(compiled["duplicate_import_rejected"], true);
}
