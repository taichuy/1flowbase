use std::sync::Arc;

use interface_runtime::{InterfaceContract, UserPrincipal};

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError, ConsoleLocaleHints,
};

pub(crate) enum FrontstagePagesInput {
    List,
    CreateGroup(CreateFrontstageGroupBody),
    CreatePage(CreateFrontstagePageBody, ConsoleLocaleHints),
    Detail(String, String, ConsoleLocaleHints),
    Update(String, UpdateFrontstagePageMetadataBody),
    Move(String, MoveFrontstagePageBody),
    Delete(String),
    ListTabs(String, ConsoleLocaleHints),
    CreateTab(String, CreateFrontstagePageTabBody),
    UpdateTab(String, String, UpdateFrontstagePageTabBody),
    DeleteTab(String, String),
    SaveDocument(String, String, SaveFrontstageTabDocumentBody),
    ListUiTemplates,
    DispatchQuery(String, String, DispatchFrontstageQueryBody),
    DispatchAction(String, String, DispatchFrontstageActionBody),
}

impl InterfaceContract for FrontstagePagesInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("List"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateGroup")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "title",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "icon",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "tooltip",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "parent_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "rank",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "placement",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("Topbar"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Sidebar"))]),
                            ]),
                        ),
                        (
                            "slug",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreatePage")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "title",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "icon",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "tooltip",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "parent_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "rank",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "placement",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("Topbar"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Sidebar"))]),
                            ]),
                        ),
                        (
                            "slug",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Detail")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Update")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[
                        (
                            "title",
                            serde_json::json!({"anyOf": [serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}), {"type":"null"}]}),
                        ),
                        (
                            "icon",
                            serde_json::json!({"anyOf": [serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}), {"type":"null"}]}),
                        ),
                        (
                            "tooltip",
                            serde_json::json!({"anyOf": [serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}), {"type":"null"}]}),
                        ),
                        (
                            "is_hidden",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]}),
                        ),
                        (
                            "placement",
                            serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Topbar"))]), mp::object_schema(&[("variant",mp::tag_schema("Sidebar"))])]), {"type":"null"}]}),
                        ),
                        (
                            "content_presentation",
                            serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Single"))]), mp::object_schema(&[("variant",mp::tag_schema("Tabs"))])]), {"type":"null"}]}),
                        ),
                        (
                            "slug",
                            serde_json::json!({"anyOf": [serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Move")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[
                        (
                            "parent_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "rank",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Delete")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListTabs")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateTab")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[
                        (
                            "title",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "route_segment",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "rank",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateTab")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "2",
                    mp::object_schema(&[
                        (
                            "title",
                            serde_json::json!({"anyOf": [serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}), {"type":"null"}]}),
                        ),
                        (
                            "rank",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteTab")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("SaveDocument")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "2",
                    mp::object_schema(&[("payload", mp::json_summary_schema())]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("ListUiTemplates"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DispatchQuery")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "2",
                    mp::object_schema(&[
                        ("query_id", mp::text_schema()),
                        ("params", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DispatchAction")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "2",
                    mp::object_schema(&[
                        ("action_id", mp::text_schema()),
                        ("params", mp::json_summary_schema()),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::List => mp::object_value(&[("variant",serde_json::Value::String("List".to_owned()))]), Self::CreateGroup(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("CreateGroup".to_owned())), ("0",mp::object_value(&[("title",match (&(_field_0).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("icon",match (&(_field_0).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("tooltip",match (&(_field_0).tooltip).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("parent_id",match (&(_field_0).parent_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("rank",match (&(_field_0).rank).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("placement",match &(_field_0).placement {crate::routes::frontstage::FrontstageNavigationPlacementResponse::Topbar => mp::object_value(&[("variant",serde_json::Value::String("Topbar".to_owned()))]), crate::routes::frontstage::FrontstageNavigationPlacementResponse::Sidebar => mp::object_value(&[("variant",serde_json::Value::String("Sidebar".to_owned()))])}), ("slug",match (&(_field_0).slug).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]), Self::CreatePage(_field_0, _) => mp::object_value(&[("variant",serde_json::Value::String("CreatePage".to_owned())), ("0",mp::object_value(&[("title",match (&(_field_0).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("icon",match (&(_field_0).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("tooltip",match (&(_field_0).tooltip).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("parent_id",match (&(_field_0).parent_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("rank",match (&(_field_0).rank).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("placement",match &(_field_0).placement {crate::routes::frontstage::FrontstageNavigationPlacementResponse::Topbar => mp::object_value(&[("variant",serde_json::Value::String("Topbar".to_owned()))]), crate::routes::frontstage::FrontstageNavigationPlacementResponse::Sidebar => mp::object_value(&[("variant",serde_json::Value::String("Sidebar".to_owned()))])}), ("slug",match (&(_field_0).slug).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]), Self::Detail(_field_0, _field_1, _) => mp::object_value(&[("variant",serde_json::Value::String("Detail".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))]))]), Self::Update(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("Update".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("title",match (&(_field_1).title).as_ref() { Some(item) => match (item).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }, None => serde_json::Value::Null }), ("icon",match (&(_field_1).icon).as_ref() { Some(item) => match (item).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }, None => serde_json::Value::Null }), ("tooltip",match (&(_field_1).tooltip).as_ref() { Some(item) => match (item).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }, None => serde_json::Value::Null }), ("is_hidden",match (&(_field_1).is_hidden).as_ref() { Some(item) => serde_json::Value::Bool(*(item)), None => serde_json::Value::Null }), ("placement",match (&(_field_1).placement).as_ref() { Some(item) => match item {crate::routes::frontstage::FrontstageNavigationPlacementResponse::Topbar => mp::object_value(&[("variant",serde_json::Value::String("Topbar".to_owned()))]), crate::routes::frontstage::FrontstageNavigationPlacementResponse::Sidebar => mp::object_value(&[("variant",serde_json::Value::String("Sidebar".to_owned()))])}, None => serde_json::Value::Null }), ("content_presentation",match (&(_field_1).content_presentation).as_ref() { Some(item) => match item {crate::routes::frontstage::FrontstagePageContentPresentationResponse::Single => mp::object_value(&[("variant",serde_json::Value::String("Single".to_owned()))]), crate::routes::frontstage::FrontstagePageContentPresentationResponse::Tabs => mp::object_value(&[("variant",serde_json::Value::String("Tabs".to_owned()))])}, None => serde_json::Value::Null }), ("slug",match (&(_field_1).slug).as_ref() { Some(item) => match (item).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }, None => serde_json::Value::Null })]))]), Self::Move(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("Move".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("parent_id",match (&(_field_1).parent_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("rank",match (&(_field_1).rank).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]), Self::Delete(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Delete".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))]))]), Self::ListTabs(_field_0, _) => mp::object_value(&[("variant",serde_json::Value::String("ListTabs".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))]))]), Self::CreateTab(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("CreateTab".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("title",match (&(_field_1).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("route_segment",match (&(_field_1).route_segment).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("rank",match (&(_field_1).rank).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]), Self::UpdateTab(_field_0, _field_1, _field_2) => mp::object_value(&[("variant",serde_json::Value::String("UpdateTab".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))])), ("2",mp::object_value(&[("title",match (&(_field_2).title).as_ref() { Some(item) => match (item).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }, None => serde_json::Value::Null }), ("rank",match (&(_field_2).rank).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]), Self::DeleteTab(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("DeleteTab".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))]))]), Self::SaveDocument(_field_0, _field_1, _field_2) => mp::object_value(&[("variant",serde_json::Value::String("SaveDocument".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))])), ("2",mp::object_value(&[("payload",mp::json_summary(&(_field_2).payload))]))]), Self::ListUiTemplates => mp::object_value(&[("variant",serde_json::Value::String("ListUiTemplates".to_owned()))]), Self::DispatchQuery(_field_0, _field_1, _field_2) => mp::object_value(&[("variant",serde_json::Value::String("DispatchQuery".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))])), ("2",mp::object_value(&[("query_id",mp::text(&(_field_2).query_id)?), ("params",mp::json_summary(&(_field_2).params))]))]), Self::DispatchAction(_field_0, _field_1, _field_2) => mp::object_value(&[("variant",serde_json::Value::String("DispatchAction".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))])), ("2",mp::object_value(&[("action_id",mp::text(&(_field_2).action_id)?), ("params",mp::json_summary(&(_field_2).params))]))])})
    }

    const CONTRACT_ID: &'static str = "console-frontstage-pages-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum FrontstagePagesOutput {
    Tree(Vec<FrontstagePageTreeNodeResponse>),
    Page(FrontstagePageResponse),
    Creation(FrontstagePageCreationResponse),
    Detail(FrontstagePageDetailResponse),
    Tabs(Vec<FrontstagePageTabResponse>),
    Tab(FrontstagePageTabResponse),
    UiTemplates(Vec<FrontstageUiTemplateResponse>),
    Json(Value),
    NoContent,
}
impl InterfaceContract for FrontstagePagesOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Tree")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("title",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("icon",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("tooltip",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("is_hidden",serde_json::json!({"type":"boolean"})), ("kind",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Group"))]), mp::object_schema(&[("variant",mp::tag_schema("Page"))])])), ("placement",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Topbar"))]), mp::object_schema(&[("variant",mp::tag_schema("Sidebar"))])])), ("content_presentation",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Single"))]), mp::object_schema(&[("variant",mp::tag_schema("Tabs"))])])), ("slug",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("children",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("title",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("icon",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("tooltip",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("is_hidden",serde_json::json!({"type":"boolean"})), ("kind",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Group"))]), mp::object_schema(&[("variant",mp::tag_schema("Page"))])])), ("placement",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Topbar"))]), mp::object_schema(&[("variant",mp::tag_schema("Sidebar"))])])), ("content_presentation",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Single"))]), mp::object_schema(&[("variant",mp::tag_schema("Tabs"))])])), ("slug",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("children",mp::object_schema(&[("item_count",mp::count_schema())]))])}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Page")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "icon",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "tooltip",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("is_hidden", serde_json::json!({"type":"boolean"})),
                        (
                            "kind",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("Group"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Page"))]),
                            ]),
                        ),
                        (
                            "parent_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "rank",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "placement",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("Topbar"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Sidebar"))]),
                            ]),
                        ),
                        (
                            "content_presentation",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("Single"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Tabs"))]),
                            ]),
                        ),
                        (
                            "slug",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Creation")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "page",
                            mp::object_schema(&[
                                ("id", mp::text_schema()),
                                (
                                    "icon",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "tooltip",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("is_hidden", serde_json::json!({"type":"boolean"})),
                                (
                                    "kind",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[("variant", mp::tag_schema("Group"))]),
                                        mp::object_schema(&[("variant", mp::tag_schema("Page"))]),
                                    ]),
                                ),
                                (
                                    "parent_id",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "rank",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "placement",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[("variant", mp::tag_schema("Topbar"))]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("Sidebar"),
                                        )]),
                                    ]),
                                ),
                                (
                                    "content_presentation",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[("variant", mp::tag_schema("Single"))]),
                                        mp::object_schema(&[("variant", mp::tag_schema("Tabs"))]),
                                    ]),
                                ),
                                (
                                    "slug",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        (
                            "default_tab",
                            mp::object_schema(&[
                                ("id", mp::text_schema()),
                                ("page_id", mp::text_schema()),
                                (
                                    "title",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "rank",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("is_default", serde_json::json!({"type":"boolean"})),
                                (
                                    "route_segment",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "document_root_uid",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Detail")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "page",
                            mp::object_schema(&[
                                ("id", mp::text_schema()),
                                (
                                    "icon",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "tooltip",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("is_hidden", serde_json::json!({"type":"boolean"})),
                                (
                                    "kind",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[("variant", mp::tag_schema("Group"))]),
                                        mp::object_schema(&[("variant", mp::tag_schema("Page"))]),
                                    ]),
                                ),
                                (
                                    "parent_id",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "rank",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "placement",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[("variant", mp::tag_schema("Topbar"))]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("Sidebar"),
                                        )]),
                                    ]),
                                ),
                                (
                                    "content_presentation",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[("variant", mp::tag_schema("Single"))]),
                                        mp::object_schema(&[("variant", mp::tag_schema("Tabs"))]),
                                    ]),
                                ),
                                (
                                    "slug",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        (
                            "tab",
                            mp::object_schema(&[
                                ("id", mp::text_schema()),
                                ("page_id", mp::text_schema()),
                                (
                                    "title",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "rank",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("is_default", serde_json::json!({"type":"boolean"})),
                                (
                                    "route_segment",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "document_root_uid",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        (
                            "document",
                            mp::object_schema(&[
                                (
                                    "root_uid",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("payload", mp::json_summary_schema()),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Tabs")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("page_id",mp::text_schema()), ("title",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("rank",mp::object_schema(&[("byte_count",mp::count_schema())])), ("is_default",serde_json::json!({"type":"boolean"})), ("route_segment",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("document_root_uid",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Tab")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("page_id", mp::text_schema()),
                        (
                            "title",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "rank",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("is_default", serde_json::json!({"type":"boolean"})),
                        (
                            "route_segment",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "document_root_uid",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UiTemplates")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("template_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("provider_code",mp::text_schema()), ("contribution_code",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("language",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Jsx"))]), mp::object_schema(&[("variant",mp::tag_schema("Tsx"))])])), ("version",mp::text_schema()), ("is_official",serde_json::json!({"type":"boolean"})), ("is_default",serde_json::json!({"type":"boolean"}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Json")),
                ("0", mp::json_summary_schema()),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("NoContent"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::Tree(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Tree".to_owned())), ("0",{ if (_field_0).len() > 32 { return None; } serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("title",match (&(item).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("icon",match (&(item).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("tooltip",match (&(item).tooltip).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("is_hidden",serde_json::Value::Bool(*(&(item).is_hidden))), ("kind",match &(item).kind {crate::routes::frontstage::FrontstagePageTreeNodeKind::Group => mp::object_value(&[("variant",serde_json::Value::String("Group".to_owned()))]), crate::routes::frontstage::FrontstagePageTreeNodeKind::Page => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned()))])}), ("placement",match &(item).placement {crate::routes::frontstage::FrontstageNavigationPlacementResponse::Topbar => mp::object_value(&[("variant",serde_json::Value::String("Topbar".to_owned()))]), crate::routes::frontstage::FrontstageNavigationPlacementResponse::Sidebar => mp::object_value(&[("variant",serde_json::Value::String("Sidebar".to_owned()))])}), ("content_presentation",match &(item).content_presentation {crate::routes::frontstage::FrontstagePageContentPresentationResponse::Single => mp::object_value(&[("variant",serde_json::Value::String("Single".to_owned()))]), crate::routes::frontstage::FrontstagePageContentPresentationResponse::Tabs => mp::object_value(&[("variant",serde_json::Value::String("Tabs".to_owned()))])}), ("slug",match (&(item).slug).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("children",{ if (&(item).children).len() > 32 { return None; } serde_json::Value::Array((&(item).children).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("title",match (&(item).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("icon",match (&(item).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("tooltip",match (&(item).tooltip).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("is_hidden",serde_json::Value::Bool(*(&(item).is_hidden))), ("kind",match &(item).kind {crate::routes::frontstage::FrontstagePageTreeNodeKind::Group => mp::object_value(&[("variant",serde_json::Value::String("Group".to_owned()))]), crate::routes::frontstage::FrontstagePageTreeNodeKind::Page => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned()))])}), ("placement",match &(item).placement {crate::routes::frontstage::FrontstageNavigationPlacementResponse::Topbar => mp::object_value(&[("variant",serde_json::Value::String("Topbar".to_owned()))]), crate::routes::frontstage::FrontstageNavigationPlacementResponse::Sidebar => mp::object_value(&[("variant",serde_json::Value::String("Sidebar".to_owned()))])}), ("content_presentation",match &(item).content_presentation {crate::routes::frontstage::FrontstagePageContentPresentationResponse::Single => mp::object_value(&[("variant",serde_json::Value::String("Single".to_owned()))]), crate::routes::frontstage::FrontstagePageContentPresentationResponse::Tabs => mp::object_value(&[("variant",serde_json::Value::String("Tabs".to_owned()))])}), ("slug",match (&(item).slug).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("children",mp::object_value(&[("item_count",serde_json::json!((&(item).children).len()))]))]))).collect::<Option<Vec<_>>>()?) })]))).collect::<Option<Vec<_>>>()?) })]), Self::Page(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned())), ("0",mp::object_value(&[("id",mp::text(&(_field_0).id)?), ("icon",match (&(_field_0).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("tooltip",match (&(_field_0).tooltip).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("is_hidden",serde_json::Value::Bool(*(&(_field_0).is_hidden))), ("kind",match &(_field_0).kind {crate::routes::frontstage::FrontstagePageTreeNodeKind::Group => mp::object_value(&[("variant",serde_json::Value::String("Group".to_owned()))]), crate::routes::frontstage::FrontstagePageTreeNodeKind::Page => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned()))])}), ("parent_id",match (&(_field_0).parent_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("rank",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).rank).len()))])), ("placement",match &(_field_0).placement {crate::routes::frontstage::FrontstageNavigationPlacementResponse::Topbar => mp::object_value(&[("variant",serde_json::Value::String("Topbar".to_owned()))]), crate::routes::frontstage::FrontstageNavigationPlacementResponse::Sidebar => mp::object_value(&[("variant",serde_json::Value::String("Sidebar".to_owned()))])}), ("content_presentation",match &(_field_0).content_presentation {crate::routes::frontstage::FrontstagePageContentPresentationResponse::Single => mp::object_value(&[("variant",serde_json::Value::String("Single".to_owned()))]), crate::routes::frontstage::FrontstagePageContentPresentationResponse::Tabs => mp::object_value(&[("variant",serde_json::Value::String("Tabs".to_owned()))])}), ("slug",match (&(_field_0).slug).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]), Self::Creation(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Creation".to_owned())), ("0",mp::object_value(&[("page",mp::object_value(&[("id",mp::text(&(&(_field_0).page).id)?), ("icon",match (&(&(_field_0).page).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("tooltip",match (&(&(_field_0).page).tooltip).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("is_hidden",serde_json::Value::Bool(*(&(&(_field_0).page).is_hidden))), ("kind",match &(&(_field_0).page).kind {crate::routes::frontstage::FrontstagePageTreeNodeKind::Group => mp::object_value(&[("variant",serde_json::Value::String("Group".to_owned()))]), crate::routes::frontstage::FrontstagePageTreeNodeKind::Page => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned()))])}), ("parent_id",match (&(&(_field_0).page).parent_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("rank",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).page).rank).len()))])), ("placement",match &(&(_field_0).page).placement {crate::routes::frontstage::FrontstageNavigationPlacementResponse::Topbar => mp::object_value(&[("variant",serde_json::Value::String("Topbar".to_owned()))]), crate::routes::frontstage::FrontstageNavigationPlacementResponse::Sidebar => mp::object_value(&[("variant",serde_json::Value::String("Sidebar".to_owned()))])}), ("content_presentation",match &(&(_field_0).page).content_presentation {crate::routes::frontstage::FrontstagePageContentPresentationResponse::Single => mp::object_value(&[("variant",serde_json::Value::String("Single".to_owned()))]), crate::routes::frontstage::FrontstagePageContentPresentationResponse::Tabs => mp::object_value(&[("variant",serde_json::Value::String("Tabs".to_owned()))])}), ("slug",match (&(&(_field_0).page).slug).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("default_tab",mp::object_value(&[("id",mp::text(&(&(_field_0).default_tab).id)?), ("page_id",mp::text(&(&(_field_0).default_tab).page_id)?), ("title",match (&(&(_field_0).default_tab).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("rank",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).default_tab).rank).len()))])), ("is_default",serde_json::Value::Bool(*(&(&(_field_0).default_tab).is_default))), ("route_segment",match (&(&(_field_0).default_tab).route_segment).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("document_root_uid",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).default_tab).document_root_uid).len()))]))]))]))]), Self::Detail(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Detail".to_owned())), ("0",mp::object_value(&[("page",mp::object_value(&[("id",mp::text(&(&(_field_0).page).id)?), ("icon",match (&(&(_field_0).page).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("tooltip",match (&(&(_field_0).page).tooltip).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("is_hidden",serde_json::Value::Bool(*(&(&(_field_0).page).is_hidden))), ("kind",match &(&(_field_0).page).kind {crate::routes::frontstage::FrontstagePageTreeNodeKind::Group => mp::object_value(&[("variant",serde_json::Value::String("Group".to_owned()))]), crate::routes::frontstage::FrontstagePageTreeNodeKind::Page => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned()))])}), ("parent_id",match (&(&(_field_0).page).parent_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("rank",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).page).rank).len()))])), ("placement",match &(&(_field_0).page).placement {crate::routes::frontstage::FrontstageNavigationPlacementResponse::Topbar => mp::object_value(&[("variant",serde_json::Value::String("Topbar".to_owned()))]), crate::routes::frontstage::FrontstageNavigationPlacementResponse::Sidebar => mp::object_value(&[("variant",serde_json::Value::String("Sidebar".to_owned()))])}), ("content_presentation",match &(&(_field_0).page).content_presentation {crate::routes::frontstage::FrontstagePageContentPresentationResponse::Single => mp::object_value(&[("variant",serde_json::Value::String("Single".to_owned()))]), crate::routes::frontstage::FrontstagePageContentPresentationResponse::Tabs => mp::object_value(&[("variant",serde_json::Value::String("Tabs".to_owned()))])}), ("slug",match (&(&(_field_0).page).slug).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("tab",mp::object_value(&[("id",mp::text(&(&(_field_0).tab).id)?), ("page_id",mp::text(&(&(_field_0).tab).page_id)?), ("title",match (&(&(_field_0).tab).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("rank",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).tab).rank).len()))])), ("is_default",serde_json::Value::Bool(*(&(&(_field_0).tab).is_default))), ("route_segment",match (&(&(_field_0).tab).route_segment).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("document_root_uid",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).tab).document_root_uid).len()))]))])), ("document",mp::object_value(&[("root_uid",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).document).root_uid).len()))])), ("payload",mp::json_summary(&(&(_field_0).document).payload))]))]))]), Self::Tabs(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Tabs".to_owned())), ("0",{ if (_field_0).len() > 32 { return None; } serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("page_id",mp::text(&(item).page_id)?), ("title",match (&(item).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("rank",mp::object_value(&[("byte_count",serde_json::json!((&(item).rank).len()))])), ("is_default",serde_json::Value::Bool(*(&(item).is_default))), ("route_segment",match (&(item).route_segment).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("document_root_uid",mp::object_value(&[("byte_count",serde_json::json!((&(item).document_root_uid).len()))]))]))).collect::<Option<Vec<_>>>()?) })]), Self::Tab(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Tab".to_owned())), ("0",mp::object_value(&[("id",mp::text(&(_field_0).id)?), ("page_id",mp::text(&(_field_0).page_id)?), ("title",match (&(_field_0).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("rank",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).rank).len()))])), ("is_default",serde_json::Value::Bool(*(&(_field_0).is_default))), ("route_segment",match (&(_field_0).route_segment).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("document_root_uid",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).document_root_uid).len()))]))]))]), Self::UiTemplates(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("UiTemplates".to_owned())), ("0",{ if (_field_0).len() > 32 { return None; } serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("template_id",match (&(item).template_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("provider_code",mp::text(&(item).provider_code)?), ("contribution_code",mp::text(&(item).contribution_code)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))])), ("source",mp::object_value(&[("byte_count",serde_json::json!((&(item).source).len()))])), ("language",match &(item).language {domain::ui_management::UiCodeTemplateLanguage::Jsx => mp::object_value(&[("variant",serde_json::Value::String("Jsx".to_owned()))]), domain::ui_management::UiCodeTemplateLanguage::Tsx => mp::object_value(&[("variant",serde_json::Value::String("Tsx".to_owned()))])}), ("version",mp::text(&(item).version)?), ("is_official",serde_json::Value::Bool(*(&(item).is_official))), ("is_default",serde_json::Value::Bool(*(&(item).is_default)))]))).collect::<Option<Vec<_>>>()?) })]), Self::Json(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Json".to_owned())), ("0",mp::json_summary(_field_0))]), Self::NoContent => mp::object_value(&[("variant",serde_json::Value::String("NoContent".to_owned()))])})
    }

    const CONTRACT_ID: &'static str = "console-frontstage-pages-output";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Clone)]
pub(crate) struct FrontstagePagesDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) bootstrap_workspace_id: Uuid,
    pub(crate) api_node_id: String,
    pub(crate) runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
}
struct FrontstagePagesAdapter(FrontstagePagesDependencies);

impl FrontstagePagesAdapter {
    async fn localize_default_tab(
        &self,
        actor: &domain::ActorContext,
        locale: ConsoleLocaleHints,
        tab: &mut domain::frontstage::FrontstagePageTabRecord,
    ) -> Result<(), ApiError> {
        if !tab.is_default {
            return Ok(());
        }
        let preferred = self
            .0
            .store
            .find_user_by_id(actor.user_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
            .preferred_locale;
        let locale = locale.resolve(preferred);
        let stored = tab.title.as_deref().unwrap_or_default();
        tab.title = Some(
            crate::app_state::project_canonical_display_with(
                &self.0.store,
                self.0.bootstrap_workspace_id,
                &locale,
                "Default",
                stored,
            )
            .await?,
        );
        Ok(())
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: FrontstagePagesInput,
    ) -> Result<FrontstagePagesOutput, ApiError> {
        let actor = principal.actor();
        let workspace_id = actor.current_workspace_id;
        let service = FrontstagePageService::for_actor(self.0.store.clone(), actor.clone());
        match input {
            FrontstagePagesInput::List => Ok(FrontstagePagesOutput::Tree(
                service
                    .list_page_tree(actor.user_id, workspace_id)
                    .await?
                    .into_iter()
                    .map(to_tree_node_response)
                    .collect(),
            )),
            FrontstagePagesInput::CreateGroup(body) => {
                let page = service
                    .create_group(CreateFrontstageGroupCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        title: body.title,
                        icon: body.icon,
                        tooltip: body.tooltip,
                        parent_id: parse_optional_uuid(body.parent_id.as_deref(), "parent_id")?,
                        rank: body.rank,
                        placement: to_domain_placement(body.placement),
                        slug: body.slug,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::Page(to_page_response(page)))
            }
            FrontstagePagesInput::CreatePage(body, locale) => {
                let creation = service
                    .create_page(CreateFrontstagePageCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        title: body.title,
                        icon: body.icon,
                        tooltip: body.tooltip,
                        parent_id: parse_optional_uuid(body.parent_id.as_deref(), "parent_id")?,
                        rank: body.rank,
                        placement: to_domain_placement(body.placement),
                        slug: body.slug,
                    })
                    .await?;
                let mut default_tab = creation.default_tab.ok_or(
                    control_plane::errors::ControlPlaneError::Conflict(
                        "frontstage_page_requires_tab",
                    ),
                )?;
                self.localize_default_tab(actor, locale, &mut default_tab)
                    .await?;
                Ok(FrontstagePagesOutput::Creation(
                    FrontstagePageCreationResponse {
                        page: to_page_response(creation.page),
                        default_tab: to_tab_response(default_tab),
                    },
                ))
            }
            FrontstagePagesInput::Detail(page_id, tab_reference, locale) => {
                let mut detail = service
                    .get_page_detail(GetFrontstagePageDetailCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        tab_reference,
                    })
                    .await?;
                self.localize_default_tab(actor, locale, &mut detail.tab)
                    .await?;
                Ok(FrontstagePagesOutput::Detail(to_page_detail_response(
                    detail,
                )))
            }
            FrontstagePagesInput::Update(page_id, body) => {
                let page = service
                    .update_metadata(UpdateFrontstagePageMetadataCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        title: body.title,
                        icon: body.icon,
                        tooltip: body.tooltip,
                        is_hidden: body.is_hidden,
                        placement: body.placement.map(to_domain_placement),
                        content_presentation: body
                            .content_presentation
                            .map(to_domain_content_presentation),
                        slug: body.slug,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::Page(to_page_response(page)))
            }
            FrontstagePagesInput::Move(page_id, body) => {
                let page = service
                    .move_page(MoveFrontstagePageCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        parent_id: parse_optional_uuid(body.parent_id.as_deref(), "parent_id")?,
                        rank: body.rank,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::Page(to_page_response(page)))
            }
            FrontstagePagesInput::Delete(page_id) => {
                service
                    .delete_page(DeleteFrontstagePageCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::NoContent)
            }
            FrontstagePagesInput::ListTabs(page_id, locale) => {
                let mut tabs = service
                    .list_page_tabs(
                        actor.user_id,
                        workspace_id,
                        parse_uuid(&page_id, "page_id")?,
                    )
                    .await?;
                for tab in &mut tabs {
                    self.localize_default_tab(actor, locale.clone(), tab)
                        .await?;
                }
                Ok(FrontstagePagesOutput::Tabs(
                    tabs.into_iter().map(to_tab_response).collect(),
                ))
            }
            FrontstagePagesInput::CreateTab(page_id, body) => {
                let tab = service
                    .create_page_tab(CreateFrontstagePageTabCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        title: body.title,
                        route_segment: body.route_segment,
                        rank: body.rank,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::Tab(to_tab_response(tab)))
            }
            FrontstagePagesInput::UpdateTab(page_id, tab_id, body) => {
                let tab = service
                    .update_page_tab(UpdateFrontstagePageTabCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        tab_id: parse_uuid(&tab_id, "tab_id")?,
                        title: body.title,
                        rank: body.rank,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::Tab(to_tab_response(tab)))
            }
            FrontstagePagesInput::DeleteTab(page_id, tab_id) => {
                service
                    .delete_page_tab(DeleteFrontstagePageTabCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        tab_id: parse_uuid(&tab_id, "tab_id")?,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::NoContent)
            }
            FrontstagePagesInput::SaveDocument(page_id, tab_id, body) => {
                let detail = service
                    .save_tab_document(SaveFrontstageTabDocumentCommand {
                        actor_user_id: actor.user_id,
                        workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        tab_id: parse_uuid(&tab_id, "tab_id")?,
                        document_payload: body.payload,
                    })
                    .await?;
                Ok(FrontstagePagesOutput::Detail(to_page_detail_response(
                    detail,
                )))
            }
            FrontstagePagesInput::ListUiTemplates => {
                if !actor.has_permission("frontstage.page.design") {
                    return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                        "frontstage.page.design",
                    )
                    .into());
                }
                let values = control_plane::ui_management::UiManagementService::new(
                    self.0.store.clone(),
                    self.0.api_node_id.clone(),
                )
                .list_published_templates_for_workspace(actor.current_workspace_id)
                .await?;
                Ok(FrontstagePagesOutput::UiTemplates(
                    values
                        .into_iter()
                        .map(|value| FrontstageUiTemplateResponse {
                            template_id: value.template_id.map(|id| id.to_string()),
                            provider_code: value.provider_code,
                            contribution_code: value.contribution_code,
                            name: value.name,
                            source: value.source,
                            language: value.language,
                            version: value.version,
                            is_official: value.is_official,
                            is_default: value.is_default,
                        })
                        .collect(),
                ))
            }
            FrontstagePagesInput::DispatchQuery(page_id, tab_id, body) => {
                let output = frontstage_query_kernel(
                    data_capabilities::FrontstageDataExecutionDependencies {
                        store: self.0.store.clone(),
                        runtime_engine: Arc::clone(&self.0.runtime_engine),
                    },
                )?
                .dispatch_json(
                    "frontstage_page_tab_query",
                    &body.query_id,
                    serde_json::to_value(FrontstageCapabilityInput {
                        actor_user_id: actor.user_id,
                        actor: actor.clone(),
                        workspace_id: actor.current_workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        tab_id: parse_uuid(&tab_id, "tab_id")?,
                        params: body.params,
                    })?,
                )
                .await?;
                Ok(FrontstagePagesOutput::Json(output))
            }
            FrontstagePagesInput::DispatchAction(page_id, tab_id, body) => {
                let output = frontstage_action_kernel(
                    data_capabilities::FrontstageDataExecutionDependencies {
                        store: self.0.store.clone(),
                        runtime_engine: Arc::clone(&self.0.runtime_engine),
                    },
                )?
                .dispatch_json(
                    "frontstage_page_tab_action",
                    &body.action_id,
                    serde_json::to_value(FrontstageCapabilityInput {
                        actor_user_id: actor.user_id,
                        actor: actor.clone(),
                        workspace_id: actor.current_workspace_id,
                        page_id: parse_uuid(&page_id, "page_id")?,
                        tab_id: parse_uuid(&tab_id, "tab_id")?,
                        params: body.params,
                    })?,
                )
                .await?;
                Ok(FrontstagePagesOutput::Json(output))
            }
        }
    }
}

impl ConsoleInterfacePort<FrontstagePagesInput, FrontstagePagesOutput> for FrontstagePagesAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: FrontstagePagesInput,
    ) -> ConsoleInterfaceFuture<'a, FrontstagePagesOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.pages.view",
        binding_id: "http.console.frontstage.pages.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.pages.create",
        binding_id: "http.console.frontstage.pages.post.v1",
        method: "POST",
        path: "/api/console/frontstage/pages",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.groups.create",
        binding_id: "http.console.frontstage.groups.post.v1",
        method: "POST",
        path: "/api/console/frontstage/pages/groups",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.pages.view",
        binding_id: "http.console.frontstage.page-detail.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/tabs/:tab_reference",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.pages.update",
        binding_id: "http.console.frontstage.pages.patch.v1",
        method: "PATCH",
        path: "/api/console/frontstage/pages/:page_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.pages.move",
        binding_id: "http.console.frontstage.pages.move.v1",
        method: "POST",
        path: "/api/console/frontstage/pages/:page_id/move",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.pages.delete",
        binding_id: "http.console.frontstage.pages.delete.v1",
        method: "DELETE",
        path: "/api/console/frontstage/pages/:page_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.tabs.view",
        binding_id: "http.console.frontstage.tabs.get.v1",
        method: "GET",
        path: "/api/console/frontstage/pages/:page_id/tabs",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.tabs.create",
        binding_id: "http.console.frontstage.tabs.post.v1",
        method: "POST",
        path: "/api/console/frontstage/pages/:page_id/tabs",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.tabs.update",
        binding_id: "http.console.frontstage.tabs.patch.v1",
        method: "PATCH",
        path: "/api/console/frontstage/pages/:page_id/tabs/:tab_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.tabs.delete",
        binding_id: "http.console.frontstage.tabs.delete.v1",
        method: "DELETE",
        path: "/api/console/frontstage/pages/:page_id/tabs/:tab_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.tabs.document.save",
        binding_id: "http.console.frontstage.tabs.document.put.v1",
        method: "PUT",
        path: "/api/console/frontstage/pages/:page_id/tabs/:tab_id/document",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.ui_templates.view",
        binding_id: "http.console.frontstage.ui-templates.get.v1",
        method: "GET",
        path: "/api/console/frontstage/ui-templates",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.queries.dispatch",
        binding_id: "http.console.frontstage.queries.dispatch.post.v1",
        method: "POST",
        path: "/api/console/frontstage/pages/:page_id/tabs/:tab_id/queries/dispatch",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.actions.dispatch",
        binding_id: "http.console.frontstage.actions.dispatch.post.v1",
        method: "POST",
        path: "/api/console/frontstage/pages/:page_id/tabs/:tab_id/actions/dispatch",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: FrontstagePagesDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-frontstage-pages",
        "graph:console-frontstage-pages-v1",
        DECLARATIONS,
        Arc::new(FrontstagePagesAdapter(dependencies)),
    )
}
