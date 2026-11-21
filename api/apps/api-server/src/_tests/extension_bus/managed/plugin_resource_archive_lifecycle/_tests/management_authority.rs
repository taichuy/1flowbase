//! Independent HTTP receipts: no browser cancellation or plugin error projection can hide these results.
use super::fixture::Fixture;
use anyhow::{Context, Result, ensure};
use reqwest::{Method, StatusCode};
use serde::Serialize;
use serde_json::{Value, json};
use std::{io::Write, os::unix::fs::OpenOptionsExt, path::Path, time::Duration};

const CAPABILITIES: &str = "/api/console/settings/roles/record-capabilities";

#[derive(Serialize)]
struct Receipt<'a> {
    case: &'a str,
    status: u16,
    body: String,
}

pub(super) async fn accept(
    f: &Fixture,
    base: &str,
    host: &str,
    rename: Value,
    output: &Path,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(12))
        .build()?;
    let mut evidence = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output.join("management-authority.ndjson"))?;
    let update_path = format!("/api/console/managed-services/ssh/hosts/{host}");
    let capabilities = |plugin: &str, id: &str| {
        json!({
            "plugin_code":plugin,"resource_code":"hosts","record_ids":[id]
        })
    };

    // Reserve every connection except one. The prior nested checkout cannot finish
    // within the HTTP deadline; acquiring a higher-capacity pool would mask the bug.
    let pool = f.runtime.store.pool();
    let mut occupied = Vec::new();
    for _ in 1..pool.options().get_max_connections() {
        occupied.push(pool.acquire().await?);
    }
    let missing = uuid::Uuid::now_v7().to_string();
    for (case, method, path, member, body, expected) in [
        (
            "root-ssh-capabilities",
            Method::POST,
            CAPABILITIES,
            false,
            capabilities("ssh", host),
            StatusCode::OK,
        ),
        (
            "root-monitor-capabilities",
            Method::POST,
            CAPABILITIES,
            false,
            capabilities("compute-monitor", host),
            StatusCode::OK,
        ),
        (
            "member-ssh-capabilities",
            Method::POST,
            CAPABILITIES,
            true,
            capabilities("ssh", host),
            StatusCode::OK,
        ),
        (
            "missing-record",
            Method::POST,
            CAPABILITIES,
            false,
            capabilities("ssh", &missing),
            StatusCode::NOT_FOUND,
        ),
        (
            "member-ssh-update",
            Method::PUT,
            &update_path,
            true,
            rename.clone(),
            StatusCode::FORBIDDEN,
        ),
        (
            "root-ssh-update",
            Method::PUT,
            &update_path,
            false,
            rename,
            StatusCode::OK,
        ),
    ] {
        if case == "root-ssh-update" {
            // The operation itself holds one separate advisory mutation lock while the
            // worker calls back. Leave exactly one other connection for authority + data.
            drop(
                occupied
                    .pop()
                    .context("test requires a second connection for the mutation lock")?,
            );
        }
        let (cookie, csrf) = f.stream_auth(member);
        let mut response = client
            .request(method, format!("{base}{path}"))
            .header("cookie", cookie)
            .header("x-csrf-token", csrf)
            .header("origin", base)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("management authority: {case}"))?;
        let status = response.status();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                bytes.len() + chunk.len() <= 16 * 1024,
                "oversize management receipt"
            );
            bytes.extend_from_slice(&chunk);
        }
        let raw = String::from_utf8(bytes)?;
        let payload: Value = serde_json::from_str(&raw)?;
        ensure!(
            !raw.contains("PRIVATE KEY")
                && !raw.contains("\"credential\"")
                && !raw.contains("\"private_key\":")
                && !raw.contains("\"passphrase\":"),
            "credential in management response"
        );
        serde_json::to_writer(
            &mut evidence,
            &Receipt {
                case,
                status: status.as_u16(),
                body: raw,
            },
        )?;
        writeln!(evidence)?;
        ensure!(
            status == expected,
            "{case}: expected {expected}, got {status}; see management-authority.ndjson"
        );
        let value = payload.get("data").unwrap_or(&payload);
        match case {
            "root-ssh-capabilities" | "root-monitor-capabilities" | "member-ssh-capabilities" => {
                let items = value["items"]
                    .as_array()
                    .context("missing capability items")?;
                ensure!(
                    items.len() == 1 && items[0]["record_id"] == host,
                    "wrong capability record"
                );
                let actions = items[0]["allowed_actions"]
                    .as_array()
                    .context("missing allowed actions")?;
                if member {
                    ensure!(
                        actions.is_empty() && items[0]["can_manage_permissions"] == false,
                        "nonroot member inherited SSH management"
                    );
                } else {
                    let action = if case == "root-ssh-capabilities" {
                        "update"
                    } else {
                        "open"
                    };
                    ensure!(
                        actions.iter().any(|a| a == action),
                        "Root missing {action}: {case}"
                    );
                }
            }
            "missing-record" => ensure!(
                payload["code"] == "record_permission_record",
                "wrong missing-record code"
            ),
            "member-ssh-update" => ensure!(
                payload["status"] == 403
                    && payload["code"] == "console_operation_permission_denied",
                "wrong permission error envelope"
            ),
            "root-ssh-update" => ensure!(
                value["host_id"] == host
                    && value["name"] == "R7 authority-verified management host",
                "unexpected update result"
            ),
            _ => unreachable!(),
        }
    }
    drop(occupied);
    Ok(())
}
