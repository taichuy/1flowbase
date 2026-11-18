use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssistantClientToolId {
    GetClientContext,
    RefreshClientView,
    ListPageBlocks,
    InspectBlockRender,
    SearchBlockRender,
    ReadBlockRenderFragment,
    ClickBlockElement,
    RecompileBlock,
}

impl AssistantClientToolId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GetClientContext => "get_client_context",
            Self::RefreshClientView => "refresh_client_view",
            Self::ListPageBlocks => "list_page_blocks",
            Self::InspectBlockRender => "inspect_block_render",
            Self::SearchBlockRender => "search_block_render",
            Self::ReadBlockRenderFragment => "read_block_render_fragment",
            Self::ClickBlockElement => "click_block_element",
            Self::RecompileBlock => "recompile_block",
        }
    }
}

#[path = "assistant/client_tools.rs"]
pub mod client_tools;
#[path = "assistant/conversation_events.rs"]
pub mod conversation_events;
pub use client_tools::AssistantClientToolBridge;
