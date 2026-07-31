use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    Router,
};
use plugin_runner::app;
use serde_json::{json, Value};
use tower::ServiceExt;

struct TempDataSourcePackage {
    root: PathBuf,
}

static NEXT_TEMP_PACKAGE_ID: AtomicU64 = AtomicU64::new(0);

impl TempDataSourcePackage {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = NEXT_TEMP_PACKAGE_ID.fetch_add(1, Ordering::Relaxed);
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "plugin-runner-data-source-tests-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative_path: &str, content: &str) {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        if relative_path == "bin/fixture_data_source" {
            let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/_fixtures/data_source/fixture_data_source.sh");
            assert_eq!(fs::read_to_string(&fixture).unwrap(), content);
            fs::hard_link(fixture, path).unwrap();
        } else {
            fs::write(path, content).unwrap();
        }
    }
}

impl Drop for TempDataSourcePackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_fixture_runtime(package: &TempDataSourcePackage) {
    let validate_output = json!({
        "ok": true,
        "result": {
            "ok": true,
            "sanitized": {
                "client_id": "***"
            }
        }
    })
    .to_string();
    let connection_output = json!({
        "ok": true,
        "result": {
            "status": "ok"
        }
    })
    .to_string();
    let catalog_output = json!({
        "ok": true,
        "result": [{
            "resource_key": "contacts",
            "display_name": "Contacts",
            "resource_kind": "object",
            "metadata": {}
        }]
    })
    .to_string();
    let describe_output = json!({
        "ok": true,
        "result": {
            "resource_key": "contacts",
            "primary_key": "id",
            "fields": [],
            "supports_preview_read": true,
            "supports_import_snapshot": true,
            "metadata": {}
        }
    })
    .to_string();
    let preview_output = json!({
        "ok": true,
        "result": {
            "rows": [{
                "id": "1",
                "email": "person@example.com"
            }],
            "next_cursor": null
        }
    })
    .to_string();
    let import_output = json!({
        "ok": true,
        "result": {
            "rows": [{
                "id": "1",
                "email": "person@example.com"
            }],
            "schema_version": "v1",
            "metadata": {}
        }
    })
    .to_string();
    let list_records_output = json!({
        "ok": true,
        "result": {
            "rows": [{
                "id": "contact-1",
                "email": "person@example.com"
            }],
            "next_cursor": null,
            "total_count": 1,
            "metadata": {
                "method": "list_records"
            }
        }
    })
    .to_string();
    let get_record_output = json!({
        "ok": true,
        "result": {
            "record": {
                "id": "contact-1",
                "email": "person@example.com"
            },
            "metadata": {
                "method": "get_record"
            }
        }
    })
    .to_string();
    let create_record_output = json!({
        "ok": true,
        "result": {
            "record": {
                "id": "contact-created",
                "email": "created@example.com"
            },
            "metadata": {
                "method": "create_record"
            }
        }
    })
    .to_string();
    let update_record_output = json!({
        "ok": true,
        "result": {
            "record": {
                "id": "contact-1",
                "email": "updated@example.com"
            },
            "metadata": {
                "method": "update_record"
            }
        }
    })
    .to_string();
    let delete_record_output = json!({
        "ok": true,
        "result": {
            "deleted": true,
            "metadata": {
                "method": "delete_record"
            }
        }
    })
    .to_string();
    let execute_sql_output = json!({
        "ok": true,
        "result": {
            "results": [
                {
                    "kind": "row_batch",
                    "columns": [{
                        "name": "value",
                        "native_type": "INT4",
                        "logical_type": "integer",
                        "encoding": "json"
                    }],
                    "rows": [[1]]
                },
                { "kind": "completion", "affected_rows": 1 }
            ]
        }
    })
    .to_string();
    let error_output = json!({
        "ok": false,
        "error": {
            "message": "unknown method",
            "provider_summary": null
        }
    })
    .to_string();

    package.write(
        "bin/fixture_data_source",
        &format!(
            r#"#!/usr/bin/env bash
set -euo pipefail

payload="$(cat)"
case "${{payload}}" in
  *'"method":"validate_config"'*)
    printf '%s' '{validate_output}'
    ;;
  *'"method":"test_connection"'*)
    printf '%s' '{connection_output}'
    ;;
  *'"method":"discover_catalog"'*)
    printf '%s' '{catalog_output}'
    ;;
  *'"method":"describe_resource"'*)
    printf '%s' '{describe_output}'
    ;;
  *'"method":"preview_read"'*)
    printf '%s' '{preview_output}'
    ;;
  *'"method":"import_snapshot"'*)
    printf '%s' '{import_output}'
    ;;
  *'"method":"list_records"'*)
    printf '%s' '{list_records_output}'
    ;;
  *'"method":"get_record"'*)
    printf '%s' '{get_record_output}'
    ;;
  *'"method":"create_record"'*)
    printf '%s' '{create_record_output}'
    ;;
  *'"method":"update_record"'*)
    printf '%s' '{update_record_output}'
    ;;
  *'"method":"delete_record"'*)
    printf '%s' '{delete_record_output}'
    ;;
  *'"method":"execute_sql"'*'-- 原样传输'*"select ';'::text as value;"*)
    printf '%s' '{execute_sql_output}'
    ;;
  *)
    printf '%s' '{error_output}'
    exit 1
    ;;
esac
"#
        ),
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = package.path().join("bin/fixture_data_source");
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn make_fixture_package() -> TempDataSourcePackage {
    let package = TempDataSourcePackage::new();
    package.write(
        "manifest.yaml",
        r#"manifest_version: 1
plugin_id: fixture_data_source@0.1.0
version: 0.1.0
vendor: taichuy
display_name: Fixture Data Source
description: Fixture Data Source
source_kind: uploaded
trust_level: unverified
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - data_source
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.data_source/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/fixture_data_source
  limits:
    memory_bytes: 134217728
    timeout_ms: 5000
node_contributions: []
"#,
    );
    package.write(
        "datasource/fixture_data_source.yaml",
        r#"source_code: fixture_data_source
display_name: Fixture Data Source
auth_modes:
  - api_key
capabilities:
  - native_sql/v1
  - validate_config
  - test_connection
  - discover_catalog
  - describe_resource
  - preview_read
  - import_snapshot
supports_sync: true
supports_webhook: false
resource_kinds:
  - object
config_schema:
  - key: client_id
    label: Client ID
    type: string
    required: true
"#,
    );
    write_fixture_runtime(&package);
    package
}

async fn request_json(app: &Router, method: Method, uri: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).unwrap()
    };
    (status, payload)
}

#[tokio::test]
async fn validates_and_discovers_catalog_through_data_source_routes() {
    let package = make_fixture_package();
    let app = app();

    let (status, load_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/load",
        json!({
            "package_root": package.path(),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let plugin_id = load_payload["plugin_id"].as_str().unwrap().to_string();

    let (status, validate_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/validate-config",
        json!({
            "plugin_id": plugin_id,
            "config_json": {
                "client_id": "abc"
            },
            "secret_json": {
                "client_secret": "secret"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(validate_payload["output"]["ok"], true);

    let (status, connection_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/test-connection",
        json!({
            "plugin_id": load_payload["plugin_id"],
            "config_json": {
                "client_id": "abc"
            },
            "secret_json": {
                "client_secret": "secret"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(connection_payload["output"]["status"], "ok");

    let (status, catalog_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/discover-catalog",
        json!({
            "plugin_id": load_payload["plugin_id"],
            "config_json": {
                "client_id": "abc"
            },
            "secret_json": {
                "client_secret": "secret"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(catalog_payload["entries"][0]["resource_key"], "contacts");

    let (status, describe_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/describe-resource",
        json!({
            "plugin_id": load_payload["plugin_id"],
            "config_json": {
                "client_id": "abc"
            },
            "secret_json": {
                "client_secret": "secret"
            },
            "resource_key": "contacts"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(describe_payload["descriptor"]["primary_key"], "id");

    let (status, preview_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/preview-read",
        json!({
            "plugin_id": load_payload["plugin_id"],
            "input": {
                "config_json": {
                    "client_id": "abc"
                },
                "secret_json": {
                    "client_secret": "secret"
                },
                "resource_key": "contacts",
                "limit": 20,
                "cursor": null,
                "options_json": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(preview_payload["rows"][0]["email"], "person@example.com");

    let (status, import_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/import-snapshot",
        json!({
            "plugin_id": load_payload["plugin_id"],
            "input": {
                "config_json": {
                    "client_id": "abc"
                },
                "secret_json": {
                    "client_secret": "secret"
                },
                "resource_key": "contacts",
                "limit": 20,
                "cursor": null,
                "options_json": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(import_payload["schema_version"], "v1");
}

#[tokio::test]
async fn ac_003_execute_sql_preserves_opaque_text_and_validates_result_contract() {
    let package = make_fixture_package();
    let app = app();
    let (status, load_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/load",
        json!({ "package_root": package.path() }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let sql = "\n-- 原样传输\nselect ';'::text as value;\n";
    let (status, payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/execute-sql",
        json!({
            "plugin_id": load_payload["plugin_id"],
            "input": {
                "config_json": { "client_id": "abc" },
                "secret_json": { "client_secret": "secret" },
                "sql": sql
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(payload["results"][0]["kind"], "row_batch");
    assert_eq!(payload["results"][0]["rows"], json!([[1]]));
    assert_eq!(payload["results"][1]["affected_rows"], 1);
}

#[tokio::test]
async fn loads_checked_in_data_source_template_package() {
    let package_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/templates/data_source_http_fixture");

    let app = app();
    let (status, payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/load",
        json!({
            "package_root": package_root,
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        payload["plugin_id"].as_str(),
        Some("data_source_http_fixture@0.1.0")
    );
}

#[tokio::test]
async fn checked_in_http_fixture_covers_rest_crud_mapping_and_header_secret_reference() {
    let package_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/templates/data_source_http_fixture");
    let definition =
        fs::read_to_string(package_root.join("datasource/data_source_http_fixture.yaml")).unwrap();
    assert!(definition.contains("list_records"));
    assert!(definition.contains("delete_record"));
    assert!(definition.contains("send_mode: secret_ref"));
    assert!(definition.contains("Authorization"));

    let app = app();
    let (status, load_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/load",
        json!({
            "package_root": package_root,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let plugin_id = load_payload["plugin_id"].as_str().unwrap().to_string();
    let connection = json!({
        "config_json": {
            "base_url": "https://api.example.test",
            "headers": [{
                "name": "Authorization",
                "value": {
                    "secret_ref": "secret:data-source-http-fixture",
                    "secret_version": 7
                }
            }]
        },
        "secret_json": {
            "authorization_header": "Bearer fixture-secret"
        }
    });

    let (status, list_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/list-records",
        json!({
            "plugin_id": plugin_id,
            "input": {
                "config_json": connection["config_json"],
                "secret_json": connection["secret_json"],
                "resource_key": "contacts",
                "context": {
                    "owner_id": "owner-1",
                    "scope_id": "scope-1"
                },
                "filters": [{
                    "field_key": "email",
                    "operator": "eq",
                    "value": "person@example.com"
                }],
                "sort": [{
                    "field_key": "created_at",
                    "descending": true
                }],
                "page": {
                    "limit": 20,
                    "cursor": "cursor-1",
                    "offset": 40
                },
                "options_json": {
                    "response_map": "items"
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list_payload["rows"][0]["email"], "person@example.com");
    assert_eq!(list_payload["next_cursor"], "cursor-2");
    assert_eq!(list_payload["metadata"]["request"]["method"], "GET");
    assert_eq!(list_payload["metadata"]["request"]["path"], "/contacts");
    assert_eq!(
        list_payload["metadata"]["request"]["query"]["email"],
        "person@example.com"
    );
    assert_eq!(
        list_payload["metadata"]["request"]["query"]["sort"],
        "-created_at"
    );
    assert_eq!(list_payload["metadata"]["request"]["query"]["limit"], 20);
    assert_eq!(
        list_payload["metadata"]["request"]["query"]["cursor"],
        "cursor-1"
    );
    assert_eq!(
        list_payload["metadata"]["request"]["header_secret_ref"],
        true
    );
    assert!(!list_payload.to_string().contains("fixture-secret"));

    let (status, get_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/get-record",
        json!({
            "plugin_id": plugin_id,
            "input": {
                "config_json": connection["config_json"],
                "secret_json": connection["secret_json"],
                "resource_key": "contacts",
                "record_id": "contact-1",
                "context": {},
                "options_json": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(get_payload["record"]["id"], "contact-1");
    assert_eq!(get_payload["metadata"]["request"]["method"], "GET");
    assert_eq!(get_payload["metadata"]["response_map"], "data");

    let (status, create_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/create-record",
        json!({
            "plugin_id": plugin_id,
            "input": {
                "config_json": connection["config_json"],
                "secret_json": connection["secret_json"],
                "resource_key": "contacts",
                "record": {
                    "email": "created@example.com"
                },
                "context": {},
                "transaction_id": null,
                "options_json": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(create_payload["record"]["email"], "created@example.com");
    assert_eq!(create_payload["metadata"]["request"]["method"], "POST");

    let (status, update_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/update-record",
        json!({
            "plugin_id": plugin_id,
            "input": {
                "config_json": connection["config_json"],
                "secret_json": connection["secret_json"],
                "resource_key": "contacts",
                "record_id": "contact-1",
                "patch": {
                    "email": "updated@example.com"
                },
                "context": {},
                "transaction_id": null,
                "options_json": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(update_payload["record"]["email"], "updated@example.com");
    assert_eq!(update_payload["metadata"]["request"]["method"], "PATCH");

    let (status, delete_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/delete-record",
        json!({
            "plugin_id": plugin_id,
            "input": {
                "config_json": connection["config_json"],
                "secret_json": connection["secret_json"],
                "resource_key": "contacts",
                "record_id": "contact-1",
                "context": {},
                "transaction_id": null,
                "options_json": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(delete_payload["deleted"], true);
    assert_eq!(delete_payload["metadata"]["request"]["method"], "DELETE");

    let (status, error_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/get-record",
        json!({
            "plugin_id": plugin_id,
            "input": {
                "config_json": connection["config_json"],
                "secret_json": connection["secret_json"],
                "resource_key": "contacts",
                "record_id": "missing",
                "context": {},
                "options_json": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(error_payload["message"]
        .as_str()
        .unwrap()
        .contains("rest_fixture_record_not_found"));
}

#[tokio::test]
async fn crud_routes_call_data_source_stdio_methods() {
    let package = make_fixture_package();
    let app = app();

    let (status, load_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/load",
        json!({
            "package_root": package.path(),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let plugin_id = load_payload["plugin_id"].as_str().unwrap().to_string();

    let connection = json!({
        "config_json": {
            "client_id": "abc"
        },
        "secret_json": {
            "client_secret": "secret"
        }
    });

    let (status, list_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/list-records",
        json!({
            "plugin_id": plugin_id,
            "input": {
                "config_json": connection["config_json"],
                "secret_json": connection["secret_json"],
                "resource_key": "contacts",
                "context": {
                    "owner_id": "owner-1",
                    "scope_id": "scope-1"
                },
                "filters": [{
                    "field_key": "email",
                    "operator": "eq",
                    "value": "person@example.com"
                }],
                "sort": [{
                    "field_key": "email",
                    "descending": false
                }],
                "page": {
                    "limit": 20,
                    "offset": 0
                },
                "options_json": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list_payload["rows"][0]["id"], "contact-1");
    assert_eq!(list_payload["metadata"]["method"], "list_records");

    let (status, get_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/get-record",
        json!({
            "plugin_id": plugin_id,
            "input": {
                "config_json": connection["config_json"],
                "secret_json": connection["secret_json"],
                "resource_key": "contacts",
                "record_id": "contact-1",
                "context": {},
                "options_json": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(get_payload["record"]["email"], "person@example.com");

    let (status, create_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/create-record",
        json!({
            "plugin_id": plugin_id,
            "input": {
                "config_json": connection["config_json"],
                "secret_json": connection["secret_json"],
                "resource_key": "contacts",
                "record": {
                    "email": "created@example.com"
                },
                "context": {},
                "transaction_id": null,
                "options_json": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(create_payload["record"]["id"], "contact-created");

    let (status, update_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/update-record",
        json!({
            "plugin_id": plugin_id,
            "input": {
                "config_json": connection["config_json"],
                "secret_json": connection["secret_json"],
                "resource_key": "contacts",
                "record_id": "contact-1",
                "patch": {
                    "email": "updated@example.com"
                },
                "context": {},
                "transaction_id": null,
                "options_json": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(update_payload["record"]["email"], "updated@example.com");

    let (status, delete_payload) = request_json(
        &app,
        Method::POST,
        "/data-sources/delete-record",
        json!({
            "plugin_id": plugin_id,
            "input": {
                "config_json": connection["config_json"],
                "secret_json": connection["secret_json"],
                "resource_key": "contacts",
                "record_id": "contact-1",
                "context": {},
                "transaction_id": null,
                "options_json": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(delete_payload["deleted"], true);
}
