use interface_runtime::{InterfaceContract, UserPrincipal};

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) enum FrontstageBlocksInput {
    Open(String, String),
    ListRoots(String, FrontstageBlockRootListQuery),
    Create(String, CreateFrontstageBlockNodeBody),
    Search(String, FrontstageBlockSearchQuery),
    Get(String, String),
    Update(String, String, UpdateFrontstageBlockNodeBody),
    UpdateDescriptors(String, String, UpdateFrontstageBlockDescriptorsBody),
    DeleteLeaf(String, String),
    Children(String, String, FrontstageBlockListQuery),
    Ancestors(String, String),
    Descendants(String, String, FrontstageBlockDescendantsQuery),
    DeleteImpact(String, String),
    Move(String, String, MoveFrontstageBlockNodeBody),
    DeleteSubtree(String, String, DeleteFrontstageBlockSubtreeBody),
    GetCode(String, String),
    GetCodeFragment(String, String, FrontstageBlockCodeFragmentQuery),
    RuntimeAssembly(String, String),
    SaveCode(String, String, SaveFrontstageBlockNodeCodeBody),
    PatchCode(String, String, PatchFrontstageBlockNodeCodeBody),
}

impl InterfaceContract for FrontstageBlocksInput {
    const CONTRACT_ID: &'static str = "console-frontstage-blocks-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed block output is projected immediately into the frontstage response"
)]
pub(crate) enum FrontstageBlocksOutput {
    Open(FrontstageBlockOpenResponse),
    Nodes(Vec<FrontstageBlockNodeResponse>),
    Node(FrontstageBlockNodeResponse),
    Search(Vec<FrontstageBlockSearchResultResponse>),
    Summaries(Vec<FrontstageBlockNodeSummaryResponse>),
    Descendants(Vec<FrontstageBlockDescendantResponse>),
    DeleteImpact(FrontstageBlockDeleteImpactResponse),
    DeleteSubtree(FrontstageBlockSubtreeDeleteResponse),
    Code(FrontstageBlockNodeCodeResponse),
    Fragment(FrontstageBlockCodeFragmentResponse),
    RuntimeAssembly(FrontstageBlockRuntimeAssemblyResponse),
    NoContent,
}

impl InterfaceContract for FrontstageBlocksOutput {
    const CONTRACT_ID: &'static str = "console-frontstage-blocks-output";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Clone)]
pub(crate) struct FrontstageBlocksDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) api_node_id: String,
}

struct FrontstageBlocksAdapter(FrontstageBlocksDependencies);

impl FrontstageBlocksAdapter {
    fn scope(
        actor: &domain::ActorContext,
        page_id: String,
        block_id: String,
    ) -> Result<FrontstageBlockScopeCommand, ApiError> {
        Ok(FrontstageBlockScopeCommand {
            actor_user_id: actor.user_id,
            workspace_id: actor.current_workspace_id,
            page_id: parse_uuid(&page_id, "page_id")?,
            block_id,
        })
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: FrontstageBlocksInput,
    ) -> Result<FrontstageBlocksOutput, ApiError> {
        let actor = principal.actor();
        let service = || FrontstagePageService::for_actor(self.0.store.clone(), actor.clone());
        let output = match input {
            FrontstageBlocksInput::Open(page_id, block_id) => {
                let target = service()
                    .open_block(Self::scope(actor, page_id, block_id)?)
                    .await?;
                FrontstageBlocksOutput::Open(FrontstageBlockOpenResponse {
                    canonical_url: format!(
                        "/{}/pages/{}/blocks/{}",
                        target.slug,
                        target.page_id,
                        encode_block_path_segment(&target.block_id)
                    ),
                })
            }
            FrontstageBlocksInput::ListRoots(page_id, query) => FrontstageBlocksOutput::Nodes(
                service()
                    .list_block_roots(ListFrontstageBlocksCommand {
                        actor_user_id: actor.user_id,
                        workspace_id: actor.current_workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        tab_id: parse_uuid(&query.tab_id, "tab_id")?,
                        limit: query.limit,
                    })
                    .await?
                    .into_iter()
                    .map(to_node_response)
                    .collect(),
            ),
            FrontstageBlocksInput::Create(page_id, body) => {
                let node = service()
                    .with_node_id(self.0.api_node_id.clone())
                    .create_block_node(CreateFrontstageBlockNodeCommand {
                        actor_user_id: actor.user_id,
                        workspace_id: actor.current_workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        tab_id: body
                            .tab_id
                            .as_deref()
                            .map(|id| parse_uuid(id, "tab_id"))
                            .transpose()?,
                        title: body.title,
                        description: body.description,
                        presentation: to_domain_presentation(body.presentation),
                        position: FrontstageBlockPosition {
                            parent_block_id: body.parent_block_id,
                            before_block_id: body.before_block_id,
                            after_block_id: body.after_block_id,
                        },
                        source_code: body.source_code,
                        input_mapping: body.input_mapping,
                        output_mapping: body.output_mapping,
                        runtime_descriptor: body.runtime_descriptor,
                    })
                    .await?;
                FrontstageBlocksOutput::Node(to_node_response(node))
            }
            FrontstageBlocksInput::Search(page_id, query) => FrontstageBlocksOutput::Search(
                service()
                    .search_blocks(SearchFrontstageBlocksCommand {
                        actor_user_id: actor.user_id,
                        workspace_id: actor.current_workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        tab_id: parse_uuid(&query.tab_id, "tab_id")?,
                        query: query.query,
                        limit: query.limit,
                    })
                    .await?
                    .into_iter()
                    .map(|result| FrontstageBlockSearchResultResponse {
                        node: to_summary_response(result.node),
                        ancestors: result
                            .ancestors
                            .into_iter()
                            .map(to_summary_response)
                            .collect(),
                    })
                    .collect(),
            ),
            FrontstageBlocksInput::Get(page_id, block_id) => {
                FrontstageBlocksOutput::Node(to_node_response(
                    service()
                        .get_block_node(Self::scope(actor, page_id, block_id)?)
                        .await?,
                ))
            }
            FrontstageBlocksInput::Update(page_id, block_id, body) => {
                FrontstageBlocksOutput::Node(to_node_response(
                    service()
                        .update_block_node(UpdateFrontstageBlockNodeCommand {
                            scope: Self::scope(actor, page_id, block_id)?,
                            title: body.title,
                            description: body.description,
                            presentation: body.presentation.map(to_domain_presentation),
                            input_mapping: body.input_mapping,
                            output_mapping: body.output_mapping,
                            runtime_descriptor: body.runtime_descriptor,
                        })
                        .await?,
                ))
            }
            FrontstageBlocksInput::UpdateDescriptors(page_id, tab_id, body) => {
                FrontstageBlocksOutput::Nodes(
                    service()
                        .update_block_descriptors(UpdateFrontstageBlockDescriptorsCommand {
                            actor_user_id: actor.user_id,
                            workspace_id: actor.current_workspace_id,
                            page_id: parse_uuid(&page_id, "page_id")?,
                            tab_id: parse_uuid(&tab_id, "tab_id")?,
                            updates: body
                                .updates
                                .into_iter()
                                .map(|item| (item.block_id, item.runtime_descriptor))
                                .collect(),
                        })
                        .await?
                        .into_iter()
                        .map(to_node_response)
                        .collect(),
                )
            }
            FrontstageBlocksInput::DeleteLeaf(page_id, block_id) => {
                service()
                    .delete_block_leaf(Self::scope(actor, page_id, block_id)?)
                    .await?;
                FrontstageBlocksOutput::NoContent
            }
            FrontstageBlocksInput::Children(page_id, block_id, query) => {
                FrontstageBlocksOutput::Summaries(
                    service()
                        .list_block_children(ListFrontstageBlockChildrenCommand {
                            scope: Self::scope(actor, page_id, block_id)?,
                            limit: query.limit,
                        })
                        .await?
                        .into_iter()
                        .map(to_summary_response)
                        .collect(),
                )
            }
            FrontstageBlocksInput::Ancestors(page_id, block_id) => {
                FrontstageBlocksOutput::Summaries(
                    service()
                        .list_block_ancestors(Self::scope(actor, page_id, block_id)?)
                        .await?
                        .into_iter()
                        .map(to_summary_response)
                        .collect(),
                )
            }
            FrontstageBlocksInput::Descendants(page_id, block_id, query) => {
                FrontstageBlocksOutput::Descendants(
                    service()
                        .list_block_descendants(ListFrontstageBlockDescendantsCommand {
                            scope: Self::scope(actor, page_id, block_id)?,
                            max_depth: query.max_depth,
                            limit: query.limit,
                        })
                        .await?
                        .into_iter()
                        .map(|projection| FrontstageBlockDescendantResponse {
                            node: to_summary_response(projection.node),
                            depth: projection.depth,
                            has_children: projection.has_children,
                            path: projection.path,
                        })
                        .collect(),
                )
            }
            FrontstageBlocksInput::DeleteImpact(page_id, block_id) => {
                let impact = service()
                    .get_block_delete_impact(Self::scope(actor, page_id, block_id)?)
                    .await?;
                FrontstageBlocksOutput::DeleteImpact(FrontstageBlockDeleteImpactResponse {
                    affected_count: impact.affected_count,
                })
            }
            FrontstageBlocksInput::Move(page_id, block_id, body) => {
                FrontstageBlocksOutput::Node(to_node_response(
                    service()
                        .move_block_node(MoveFrontstageBlockNodeCommand {
                            scope: Self::scope(actor, page_id, block_id)?,
                            position: FrontstageBlockPosition {
                                parent_block_id: body.parent_block_id,
                                before_block_id: body.before_block_id,
                                after_block_id: body.after_block_id,
                            },
                        })
                        .await?,
                ))
            }
            FrontstageBlocksInput::DeleteSubtree(page_id, block_id, body) => {
                let deleted = service()
                    .delete_block_subtree(DeleteFrontstageBlockSubtreeCommand {
                        scope: Self::scope(actor, page_id, block_id)?,
                        expected_affected_count: body.expected_affected_count,
                    })
                    .await?;
                FrontstageBlocksOutput::DeleteSubtree(FrontstageBlockSubtreeDeleteResponse {
                    deleted_count: deleted.deleted_count,
                })
            }
            FrontstageBlocksInput::GetCode(page_id, block_id) => {
                let scope = Self::scope(actor, page_id, block_id)?;
                let public_block_id = scope.block_id.clone();
                FrontstageBlocksOutput::Code(to_code_response(
                    public_block_id,
                    service().get_block_node_code(scope).await?,
                ))
            }
            FrontstageBlocksInput::GetCodeFragment(page_id, block_id, query) => {
                let fragment = service()
                    .get_block_code_fragment(GetFrontstageBlockCodeFragmentCommand {
                        scope: Self::scope(actor, page_id, block_id)?,
                        start_line: query.start_line,
                        start_column: query.start_column,
                        line_count: query.line_count,
                        max_chars: query.max_chars,
                    })
                    .await?;
                FrontstageBlocksOutput::Fragment(FrontstageBlockCodeFragmentResponse {
                    block_id: fragment.block_id,
                    page_id: fragment.page_id.to_string(),
                    source_revision: fragment.source_revision,
                    source_fragment: fragment.source_fragment,
                    start_line: fragment.start_line,
                    start_column: fragment.start_column,
                    end_line: fragment.end_line,
                    end_column: fragment.end_column,
                    total_lines: fragment.total_lines,
                    total_chars: fragment.total_chars,
                    next_line: fragment.next_line,
                    next_column: fragment.next_column,
                    truncated_by_max_chars: fragment.truncated_by_max_chars,
                })
            }
            FrontstageBlocksInput::RuntimeAssembly(page_id, block_id) => {
                FrontstageBlocksOutput::RuntimeAssembly(FrontstageBlockRuntimeAssemblyResponse {
                    layers: service()
                        .get_block_runtime_assembly(Self::scope(actor, page_id, block_id)?)
                        .await?
                        .into_iter()
                        .map(to_runtime_layer_response)
                        .collect(),
                })
            }
            FrontstageBlocksInput::SaveCode(page_id, block_id, body) => {
                let scope = Self::scope(actor, page_id, block_id)?;
                let public_block_id = scope.block_id.clone();
                let code = service()
                    .with_node_id(self.0.api_node_id.clone())
                    .save_block_node_code(SaveFrontstageBlockNodeCodeCommand {
                        scope,
                        expected_source_revision: body.expected_source_revision,
                        source_code: body.source_code,
                    })
                    .await?;
                FrontstageBlocksOutput::Code(to_code_response(public_block_id, code))
            }
            FrontstageBlocksInput::PatchCode(page_id, block_id, body) => {
                let scope = Self::scope(actor, page_id, block_id)?;
                let public_block_id = scope.block_id.clone();
                let code = service()
                    .with_node_id(self.0.api_node_id.clone())
                    .patch_block_node_code(PatchFrontstageBlockNodeCodeCommand {
                        scope,
                        expected_source_revision: body.expected_source_revision,
                        edits: body
                            .edits
                            .into_iter()
                            .map(|edit| FrontstageSourceEdit {
                                start_line: edit.start_line,
                                start_column: edit.start_column,
                                end_line: edit.end_line,
                                end_column: edit.end_column,
                                replacement: edit.replacement,
                            })
                            .collect(),
                    })
                    .await?;
                FrontstageBlocksOutput::Code(to_code_response(public_block_id, code))
            }
        };
        Ok(output)
    }
}

impl ConsoleInterfacePort<FrontstageBlocksInput, FrontstageBlocksOutput>
    for FrontstageBlocksAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: FrontstageBlocksInput,
    ) -> ConsoleInterfaceFuture<'a, FrontstageBlocksOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.open",
        binding_id: "http.console.frontstage.blocks.open.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id/open",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.view",
        binding_id: "http.console.frontstage.blocks.list.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/blocks",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.create",
        binding_id: "http.console.frontstage.blocks.create.post.v1",
        method: "POST",
        path: "/api/console/frontstage/pages/:page_id/blocks",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.search",
        binding_id: "http.console.frontstage.blocks.search.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/blocks/search",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.view",
        binding_id: "http.console.frontstage.blocks.detail.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.update",
        binding_id: "http.console.frontstage.blocks.update.patch.v1",
        method: "PATCH",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.update",
        binding_id: "http.console.frontstage.blocks.descriptors.put.v1",
        method: "PUT",
        path: "/api/console/frontstage/pages/:page_id/tabs/:tab_id/block-descriptors",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.delete",
        binding_id: "http.console.frontstage.blocks.delete.delete.v1",
        method: "DELETE",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.view",
        binding_id: "http.console.frontstage.blocks.children.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id/children",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.view",
        binding_id: "http.console.frontstage.blocks.ancestors.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id/ancestors",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.view",
        binding_id: "http.console.frontstage.blocks.descendants.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id/descendants",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.view",
        binding_id: "http.console.frontstage.blocks.delete-impact.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id/delete-impact",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.move",
        binding_id: "http.console.frontstage.blocks.move.post.v1",
        method: "POST",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id/move",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.delete",
        binding_id: "http.console.frontstage.blocks.delete-subtree.post.v1",
        method: "POST",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id/delete-subtree",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.code.view",
        binding_id: "http.console.frontstage.blocks.code.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id/code",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.code.view",
        binding_id: "http.console.frontstage.blocks.code-fragment.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id/code/fragment",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.runtime.view",
        binding_id: "http.console.frontstage.blocks.runtime-assembly.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id/runtime-assembly",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.code.update",
        binding_id: "http.console.frontstage.blocks.code.put.v1",
        method: "PUT",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id/code",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.blocks.code.update",
        binding_id: "http.console.frontstage.blocks.code.patch.v1",
        method: "PATCH",
        path: "/api/console/frontstage/pages/:page_id/blocks/:block_id/code",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: FrontstageBlocksDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-frontstage-blocks",
        "graph:console-frontstage-blocks-v1",
        DECLARATIONS,
        Arc::new(FrontstageBlocksAdapter(dependencies)),
    )
}
