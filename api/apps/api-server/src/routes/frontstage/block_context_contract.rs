use std::{collections::BTreeSet, sync::Arc};

use anyhow::{bail, Context};
use axum::{extract::State, http::HeaderMap, Json};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    },
};

const BLOCK_CONTEXT_CONTRACT_BYTES: &[u8] =
    include_bytes!("../../../resources/ctx/block-context.v1.json");
const BLOCK_CONTEXT_SCHEMA_VERSION: &str = "1flowbase.block-context-contract/v1";
const BLOCK_CONTEXT_CONTRACT_VERSION: &str = "1.0.0";
const BLOCK_SDK_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlockContextEntryKind {
    Object,
    Function,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlockContextMemberKind {
    Property,
    Method,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BlockContextMemberResponse {
    pub name: String,
    pub kind: BlockContextMemberKind,
    #[schema(rename = "type")]
    #[serde(rename = "type")]
    pub type_name: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BlockContextEntryResponse {
    pub key: String,
    pub kind: BlockContextEntryKind,
    pub nullable: bool,
    #[schema(rename = "type")]
    #[serde(rename = "type")]
    pub type_name: String,
    pub description: String,
    pub members: Vec<BlockContextMemberResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BlockContextContractResponse {
    pub schema_version: String,
    pub contract_version: String,
    pub block_sdk_version: String,
    pub entries: Vec<BlockContextEntryResponse>,
    pub non_context_symbols: Vec<String>,
}

impl InterfaceContract for BlockContextContractResponse {
    const CONTRACT_ID: &'static str = "console-frontstage-block-context-contract-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) fn decode_block_context_contract(
    bytes: &[u8],
) -> anyhow::Result<BlockContextContractResponse> {
    let contract: BlockContextContractResponse =
        serde_json::from_slice(bytes).context("invalid embedded BlockContext contract JSON")?;
    if contract.schema_version != BLOCK_CONTEXT_SCHEMA_VERSION {
        bail!("unsupported BlockContext contract schema version");
    }
    if contract.contract_version != BLOCK_CONTEXT_CONTRACT_VERSION {
        bail!("unsupported BlockContext contract version");
    }
    if contract.block_sdk_version != BLOCK_SDK_VERSION {
        bail!("BlockContext contract and Block SDK versions differ");
    }
    if contract.entries.len() != 17 {
        bail!("BlockContext contract must expose exactly 17 top-level entries");
    }
    validate_named_items(
        contract.entries.iter().map(|entry| entry.key.as_str()),
        "BlockContext entry",
    )?;
    validate_named_items(
        contract.non_context_symbols.iter().map(String::as_str),
        "non-context symbol",
    )?;
    for entry in &contract.entries {
        if entry.type_name.trim().is_empty() || entry.description.trim().is_empty() {
            bail!("BlockContext entries require type and description");
        }
        validate_named_items(
            entry.members.iter().map(|member| member.name.as_str()),
            "BlockContext member",
        )?;
        if entry.members.iter().any(|member| {
            member.type_name.trim().is_empty() || member.description.trim().is_empty()
        }) {
            bail!("BlockContext members require type and description");
        }
    }
    Ok(contract)
}

fn validate_named_items<'a>(
    names: impl IntoIterator<Item = &'a str>,
    label: &str,
) -> anyhow::Result<()> {
    let mut unique = BTreeSet::new();
    for name in names {
        if name.trim().is_empty() || !unique.insert(name) {
            bail!("{label} names must be non-empty and unique");
        }
    }
    Ok(())
}

fn embedded_block_context_contract() -> Arc<BlockContextContractResponse> {
    Arc::new(
        decode_block_context_contract(BLOCK_CONTEXT_CONTRACT_BYTES)
            .expect("embedded BlockContext contract must be valid at API startup"),
    )
}

pub struct BlockContextContractInput;

impl InterfaceContract for BlockContextContractInput {
    const CONTRACT_ID: &'static str = "console-frontstage-block-context-contract-input";
    const CONTRACT_VERSION: &'static str = "1";
}

struct BlockContextContractAdapter(Arc<BlockContextContractResponse>);

impl ConsoleInterfacePort<BlockContextContractInput, BlockContextContractResponse>
    for BlockContextContractAdapter
{
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: BlockContextContractInput,
    ) -> ConsoleInterfaceFuture<'a, BlockContextContractResponse> {
        let contract = Arc::clone(&self.0);
        Box::pin(async move { Ok((*contract).clone()) })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[ConsoleInterfaceDeclaration {
    interface_id: "frontstage.block_context.view",
    binding_id: "http.console.frontstage.block-context-contract.get.v1",
    method: "GET",
    path: "/api/console/frontstage/block-context-contract",
    mutating: false,
}];

pub(crate) fn compile_registry() -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-frontstage-block-context-contract",
        "graph:console-frontstage-block-context-contract-v1",
        DECLARATIONS,
        Arc::new(BlockContextContractAdapter(
            embedded_block_context_contract(),
        )),
    )
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/block-context-contract",
    operation_id = "get_frontstage_block_context_contract",
    summary = "Get the Frontstage BlockContext contract",
    description = "Returns the versioned Host runtime contract exposed to frontend Blocks as ctx.* capabilities.",
    responses(
        (status = 200, body = BlockContextContractResponse),
        (status = 401, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_frontstage_block_context_contract(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<BlockContextContractResponse>>, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let contract = console_interface::invoke(
        snapshot_state,
        "http.console.frontstage.block-context-contract.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        BlockContextContractInput,
    )
    .await?;
    Ok(Json(ApiSuccess::new(contract)))
}
