use std::{env, fs, path::PathBuf};

fn between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start = source.find(start).expect("production fixture start marker");
    let end = start
        + source[start..]
            .find(end)
            .expect("production fixture end marker");
    &source[start..end]
}

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let fixture = root
        .join("../../../../crates/control-plane/src/application_public_api/test_support/mod.rs")
        .canonicalize()
        .unwrap();
    println!(
        "cargo:rerun-if-changed={}",
        fixture.parent().unwrap().display()
    );
    let fixture_source = format!("pub use control_plane::application_public_api::*;\n#[path = {fixture:?}] mod test_support;\npub use test_support::ApplicationPublicApiTestHarness;\n");
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("application_fixtures.rs"),
        fixture_source,
    )
    .unwrap();
    let native = root
        .join("../../src/routes/application_public_api/native.rs")
        .canonicalize()
        .unwrap();
    let projection = native
        .parent()
        .unwrap()
        .join("native/response_projection.rs");
    println!("cargo:rerun-if-changed={}", native.display());
    println!("cargo:rerun-if-changed={}", projection.display());
    let source = fs::read_to_string(native).unwrap();
    // Compile the verbatim production DTO/error response items and the entire projection
    // module. This omits the unrelated composition root; it supplies no replacement policy.
    let declarations = between(
        &source,
        "#[derive(Debug, Serialize, ToSchema)]\npub struct NativeModelObject",
        "#[derive(Debug)]\npub(crate) struct NativeRunRequestParseError",
    );
    let response = between(
        &source,
        "impl NativeApiError {",
        "pub(crate) fn bearer_token(",
    );
    let generated = format!(
        r#"
use axum::{{http::StatusCode, response::{{IntoResponse, Response}}, Json}};
use control_plane::application_public_api::{{
    native::{{NativeRunResult, NativeRunStatus, NativeRunValidationError}},
    model_catalog::{{AgentModelCapabilities, AgentModelDescriptor, AgentModelReasoning}},
}};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;
{declarations}
{response}
#[path = {projection:?}]
mod response_projection;
"#
    );
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("native_projection.rs"),
        generated,
    )
    .unwrap();
}
