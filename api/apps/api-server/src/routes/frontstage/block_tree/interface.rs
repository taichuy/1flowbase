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
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Open")),
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
                ("variant", mp::tag_schema("ListRoots")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[
                        ("tab_id", mp::text_schema()),
                        ("limit", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Create")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[
                        (
                            "tab_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "title",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "description",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "presentation",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("Page"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Drawer"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Modal"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Inline"))]),
                            ]),
                        ),
                        (
                            "parent_block_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "before_block_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "after_block_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("source_code", mp::text_schema()),
                        (
                            "input_mapping",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                        (
                            "output_mapping",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                        (
                            "runtime_descriptor",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Search")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[
                        ("tab_id", mp::text_schema()),
                        (
                            "query",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("limit", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Get")),
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
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "2",
                    mp::object_schema(&[
                        (
                            "title",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "description",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "presentation",
                            serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Page"))]), mp::object_schema(&[("variant",mp::tag_schema("Drawer"))]), mp::object_schema(&[("variant",mp::tag_schema("Modal"))]), mp::object_schema(&[("variant",mp::tag_schema("Inline"))])]), {"type":"null"}]}),
                        ),
                        (
                            "input_mapping",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("item_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "output_mapping",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("item_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "runtime_descriptor",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateDescriptors")),
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
                    mp::object_schema(&[(
                        "updates",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("block_id",mp::text_schema()), ("runtime_descriptor",mp::json_summary_schema())])}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteLeaf")),
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
                ("variant", mp::tag_schema("Children")),
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
                    mp::object_schema(&[("limit", serde_json::json!({"type":"integer"}))]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Ancestors")),
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
                ("variant", mp::tag_schema("Descendants")),
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
                        ("max_depth", serde_json::json!({"type":"integer"})),
                        ("limit", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteImpact")),
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
                ("variant", mp::tag_schema("Move")),
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
                            "parent_block_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "before_block_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "after_block_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteSubtree")),
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
                    mp::object_schema(&[(
                        "expected_affected_count",
                        serde_json::json!({"type":"integer"}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetCode")),
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
                ("variant", mp::tag_schema("GetCodeFragment")),
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
                        ("start_line", serde_json::json!({"type":"integer"})),
                        ("start_column", serde_json::json!({"type":"integer"})),
                        ("line_count", serde_json::json!({"type":"integer"})),
                        ("max_chars", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RuntimeAssembly")),
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
                ("variant", mp::tag_schema("SaveCode")),
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
                            "expected_source_revision",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("source_code", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PatchCode")),
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
                            "expected_source_revision",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "edits",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("start_line",serde_json::json!({"type":"integer"})), ("start_column",serde_json::json!({"type":"integer"})), ("end_line",serde_json::json!({"type":"integer"})), ("end_column",serde_json::json!({"type":"integer"})), ("replacement",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::Open(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("Open".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))]))]), Self::ListRoots(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("ListRoots".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("tab_id",mp::text(&(_field_1).tab_id)?), ("limit",serde_json::json!(*(&(_field_1).limit)))]))]), Self::Create(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("Create".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("tab_id",match (&(_field_1).tab_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("title",match (&(_field_1).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("description",match (&(_field_1).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("presentation",match &(_field_1).presentation {crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Page => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Drawer => mp::object_value(&[("variant",serde_json::Value::String("Drawer".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Modal => mp::object_value(&[("variant",serde_json::Value::String("Modal".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Inline => mp::object_value(&[("variant",serde_json::Value::String("Inline".to_owned()))])}), ("parent_block_id",match (&(_field_1).parent_block_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("before_block_id",match (&(_field_1).before_block_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("after_block_id",match (&(_field_1).after_block_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("source_code",mp::text(&(_field_1).source_code)?), ("input_mapping",mp::object_value(&[("item_count",serde_json::json!((&(_field_1).input_mapping).len()))])), ("output_mapping",mp::object_value(&[("item_count",serde_json::json!((&(_field_1).output_mapping).len()))])), ("runtime_descriptor",match (&(_field_1).runtime_descriptor).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null })]))]), Self::Search(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("Search".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("tab_id",mp::text(&(_field_1).tab_id)?), ("query",mp::object_value(&[("byte_count",serde_json::json!((&(_field_1).query).len()))])), ("limit",serde_json::json!(*(&(_field_1).limit)))]))]), Self::Get(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("Get".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))]))]), Self::Update(_field_0, _field_1, _field_2) => mp::object_value(&[("variant",serde_json::Value::String("Update".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))])), ("2",mp::object_value(&[("title",match (&(_field_2).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("description",match (&(_field_2).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("presentation",match (&(_field_2).presentation).as_ref() { Some(item) => match item {crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Page => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Drawer => mp::object_value(&[("variant",serde_json::Value::String("Drawer".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Modal => mp::object_value(&[("variant",serde_json::Value::String("Modal".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Inline => mp::object_value(&[("variant",serde_json::Value::String("Inline".to_owned()))])}, None => serde_json::Value::Null }), ("input_mapping",match (&(_field_2).input_mapping).as_ref() { Some(item) => mp::object_value(&[("item_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("output_mapping",match (&(_field_2).output_mapping).as_ref() { Some(item) => mp::object_value(&[("item_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("runtime_descriptor",match (&(_field_2).runtime_descriptor).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null })]))]), Self::UpdateDescriptors(_field_0, _field_1, _field_2) => mp::object_value(&[("variant",serde_json::Value::String("UpdateDescriptors".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))])), ("2",mp::object_value(&[("updates",{ if (&(_field_2).updates).len() > 32 { return None; } serde_json::Value::Array((&(_field_2).updates).iter().map(|item| Some(mp::object_value(&[("block_id",mp::text(&(item).block_id)?), ("runtime_descriptor",mp::json_summary(&(item).runtime_descriptor))]))).collect::<Option<Vec<_>>>()?) })]))]), Self::DeleteLeaf(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("DeleteLeaf".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))]))]), Self::Children(_field_0, _field_1, _field_2) => mp::object_value(&[("variant",serde_json::Value::String("Children".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))])), ("2",mp::object_value(&[("limit",serde_json::json!(*(&(_field_2).limit)))]))]), Self::Ancestors(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("Ancestors".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))]))]), Self::Descendants(_field_0, _field_1, _field_2) => mp::object_value(&[("variant",serde_json::Value::String("Descendants".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))])), ("2",mp::object_value(&[("max_depth",serde_json::json!(*(&(_field_2).max_depth))), ("limit",serde_json::json!(*(&(_field_2).limit)))]))]), Self::DeleteImpact(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("DeleteImpact".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))]))]), Self::Move(_field_0, _field_1, _field_2) => mp::object_value(&[("variant",serde_json::Value::String("Move".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))])), ("2",mp::object_value(&[("parent_block_id",match (&(_field_2).parent_block_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("before_block_id",match (&(_field_2).before_block_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("after_block_id",match (&(_field_2).after_block_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })]))]), Self::DeleteSubtree(_field_0, _field_1, _field_2) => mp::object_value(&[("variant",serde_json::Value::String("DeleteSubtree".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))])), ("2",mp::object_value(&[("expected_affected_count",serde_json::json!(*(&(_field_2).expected_affected_count)))]))]), Self::GetCode(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("GetCode".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))]))]), Self::GetCodeFragment(_field_0, _field_1, _field_2) => mp::object_value(&[("variant",serde_json::Value::String("GetCodeFragment".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))])), ("2",mp::object_value(&[("start_line",serde_json::json!(*(&(_field_2).start_line))), ("start_column",serde_json::json!(*(&(_field_2).start_column))), ("line_count",serde_json::json!(*(&(_field_2).line_count))), ("max_chars",serde_json::json!(*(&(_field_2).max_chars)))]))]), Self::RuntimeAssembly(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("RuntimeAssembly".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))]))]), Self::SaveCode(_field_0, _field_1, _field_2) => mp::object_value(&[("variant",serde_json::Value::String("SaveCode".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))])), ("2",mp::object_value(&[("expected_source_revision",match (&(_field_2).expected_source_revision).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("source_code",mp::text(&(_field_2).source_code)?)]))]), Self::PatchCode(_field_0, _field_1, _field_2) => mp::object_value(&[("variant",serde_json::Value::String("PatchCode".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("byte_count",serde_json::json!((_field_1).len()))])), ("2",mp::object_value(&[("expected_source_revision",mp::object_value(&[("byte_count",serde_json::json!((&(_field_2).expected_source_revision).len()))])), ("edits",{ if (&(_field_2).edits).len() > 32 { return None; } serde_json::Value::Array((&(_field_2).edits).iter().map(|item| Some(mp::object_value(&[("start_line",serde_json::json!(*(&(item).start_line))), ("start_column",serde_json::json!(*(&(item).start_column))), ("end_line",serde_json::json!(*(&(item).end_line))), ("end_column",serde_json::json!(*(&(item).end_column))), ("replacement",mp::object_value(&[("byte_count",serde_json::json!((&(item).replacement).len()))]))]))).collect::<Option<Vec<_>>>()?) })]))])})
    }

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
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("Open"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Nodes")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("block_id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("page_id",mp::text_schema()), ("tab_id",mp::text_schema()), ("parent_block_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("rank",mp::object_schema(&[("byte_count",mp::count_schema())])), ("presentation",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Page"))]), mp::object_schema(&[("variant",mp::tag_schema("Drawer"))]), mp::object_schema(&[("variant",mp::tag_schema("Modal"))]), mp::object_schema(&[("variant",mp::tag_schema("Inline"))])])), ("title",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("schema_version",serde_json::json!({"type":"integer"})), ("input_mapping",mp::object_schema(&[("item_count",mp::count_schema())])), ("output_mapping",mp::object_schema(&[("item_count",mp::count_schema())])), ("runtime_descriptor",mp::json_summary_schema()), ("code_ref",mp::object_schema(&[("byte_count",mp::count_schema())])), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Node")),
                (
                    "0",
                    mp::object_schema(&[
                        ("block_id", mp::text_schema()),
                        ("workspace_id", mp::text_schema()),
                        ("page_id", mp::text_schema()),
                        ("tab_id", mp::text_schema()),
                        (
                            "parent_block_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "rank",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "presentation",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("Page"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Drawer"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Modal"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Inline"))]),
                            ]),
                        ),
                        (
                            "title",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "description",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("schema_version", serde_json::json!({"type":"integer"})),
                        (
                            "input_mapping",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                        (
                            "output_mapping",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                        ("runtime_descriptor", mp::json_summary_schema()),
                        (
                            "code_ref",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Search")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("node",mp::object_schema(&[("block_id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("page_id",mp::text_schema()), ("tab_id",mp::text_schema()), ("parent_block_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("rank",mp::object_schema(&[("byte_count",mp::count_schema())])), ("presentation",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Page"))]), mp::object_schema(&[("variant",mp::tag_schema("Drawer"))]), mp::object_schema(&[("variant",mp::tag_schema("Modal"))]), mp::object_schema(&[("variant",mp::tag_schema("Inline"))])])), ("title",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("schema_version",serde_json::json!({"type":"integer"})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])), ("ancestors",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("block_id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("page_id",mp::text_schema()), ("tab_id",mp::text_schema()), ("parent_block_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("rank",mp::object_schema(&[("byte_count",mp::count_schema())])), ("presentation",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Page"))]), mp::object_schema(&[("variant",mp::tag_schema("Drawer"))]), mp::object_schema(&[("variant",mp::tag_schema("Modal"))]), mp::object_schema(&[("variant",mp::tag_schema("Inline"))])])), ("title",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("schema_version",serde_json::json!({"type":"integer"})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Summaries")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("block_id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("page_id",mp::text_schema()), ("tab_id",mp::text_schema()), ("parent_block_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("rank",mp::object_schema(&[("byte_count",mp::count_schema())])), ("presentation",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Page"))]), mp::object_schema(&[("variant",mp::tag_schema("Drawer"))]), mp::object_schema(&[("variant",mp::tag_schema("Modal"))]), mp::object_schema(&[("variant",mp::tag_schema("Inline"))])])), ("title",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("schema_version",serde_json::json!({"type":"integer"})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Descendants")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("node",mp::object_schema(&[("block_id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("page_id",mp::text_schema()), ("tab_id",mp::text_schema()), ("parent_block_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("rank",mp::object_schema(&[("byte_count",mp::count_schema())])), ("presentation",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Page"))]), mp::object_schema(&[("variant",mp::tag_schema("Drawer"))]), mp::object_schema(&[("variant",mp::tag_schema("Modal"))]), mp::object_schema(&[("variant",mp::tag_schema("Inline"))])])), ("title",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("schema_version",serde_json::json!({"type":"integer"})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])), ("depth",serde_json::json!({"type":"integer"})), ("has_children",serde_json::json!({"type":"boolean"})), ("path",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteImpact")),
                (
                    "0",
                    mp::object_schema(&[("affected_count", serde_json::json!({"type":"integer"}))]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteSubtree")),
                (
                    "0",
                    mp::object_schema(&[("deleted_count", serde_json::json!({"type":"integer"}))]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Code")),
                (
                    "0",
                    mp::object_schema(&[
                        ("block_id", mp::text_schema()),
                        ("page_id", mp::text_schema()),
                        ("source_code", mp::text_schema()),
                        (
                            "source_sha256",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Fragment")),
                (
                    "0",
                    mp::object_schema(&[
                        ("block_id", mp::text_schema()),
                        ("page_id", mp::text_schema()),
                        (
                            "source_revision",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "source_fragment",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("start_line", serde_json::json!({"type":"integer"})),
                        ("start_column", serde_json::json!({"type":"integer"})),
                        ("end_line", serde_json::json!({"type":"integer"})),
                        ("end_column", serde_json::json!({"type":"integer"})),
                        ("total_lines", serde_json::json!({"type":"integer"})),
                        ("total_chars", serde_json::json!({"type":"integer"})),
                        (
                            "next_line",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "next_column",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "truncated_by_max_chars",
                            serde_json::json!({"type":"boolean"}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RuntimeAssembly")),
                (
                    "0",
                    mp::object_schema(&[(
                        "layers",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("block_id",mp::text_schema()), ("tab_id",mp::text_schema()), ("parent_block_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("title",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("presentation",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Page"))]), mp::object_schema(&[("variant",mp::tag_schema("Drawer"))]), mp::object_schema(&[("variant",mp::tag_schema("Modal"))]), mp::object_schema(&[("variant",mp::tag_schema("Inline"))])])), ("schema_version",serde_json::json!({"type":"integer"})), ("input_mapping",mp::object_schema(&[("item_count",mp::count_schema())])), ("output_mapping",mp::object_schema(&[("item_count",mp::count_schema())])), ("runtime_descriptor",mp::json_summary_schema()), ("code_ref",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source_revision",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                    )]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("NoContent"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::Open(_) => mp::object_value(&[("variant",serde_json::Value::String("Open".to_owned()))]), Self::Nodes(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Nodes".to_owned())), ("0",{ if (_field_0).len() > 32 { return None; } serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("block_id",mp::text(&(item).block_id)?), ("workspace_id",mp::text(&(item).workspace_id)?), ("page_id",mp::text(&(item).page_id)?), ("tab_id",mp::text(&(item).tab_id)?), ("parent_block_id",match (&(item).parent_block_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("rank",mp::object_value(&[("byte_count",serde_json::json!((&(item).rank).len()))])), ("presentation",match &(item).presentation {crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Page => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Drawer => mp::object_value(&[("variant",serde_json::Value::String("Drawer".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Modal => mp::object_value(&[("variant",serde_json::Value::String("Modal".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Inline => mp::object_value(&[("variant",serde_json::Value::String("Inline".to_owned()))])}), ("title",match (&(item).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("description",match (&(item).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("schema_version",serde_json::json!(*(&(item).schema_version))), ("input_mapping",mp::object_value(&[("item_count",serde_json::json!((&(item).input_mapping).len()))])), ("output_mapping",mp::object_value(&[("item_count",serde_json::json!((&(item).output_mapping).len()))])), ("runtime_descriptor",mp::json_summary(&(item).runtime_descriptor)), ("code_ref",mp::object_value(&[("byte_count",serde_json::json!((&(item).code_ref).len()))])), ("created_at",mp::text(&(item).created_at)?), ("updated_at",mp::text(&(item).updated_at)?)]))).collect::<Option<Vec<_>>>()?) })]), Self::Node(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Node".to_owned())), ("0",mp::object_value(&[("block_id",mp::text(&(_field_0).block_id)?), ("workspace_id",mp::text(&(_field_0).workspace_id)?), ("page_id",mp::text(&(_field_0).page_id)?), ("tab_id",mp::text(&(_field_0).tab_id)?), ("parent_block_id",match (&(_field_0).parent_block_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("rank",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).rank).len()))])), ("presentation",match &(_field_0).presentation {crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Page => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Drawer => mp::object_value(&[("variant",serde_json::Value::String("Drawer".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Modal => mp::object_value(&[("variant",serde_json::Value::String("Modal".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Inline => mp::object_value(&[("variant",serde_json::Value::String("Inline".to_owned()))])}), ("title",match (&(_field_0).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("description",match (&(_field_0).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("schema_version",serde_json::json!(*(&(_field_0).schema_version))), ("input_mapping",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).input_mapping).len()))])), ("output_mapping",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).output_mapping).len()))])), ("runtime_descriptor",mp::json_summary(&(_field_0).runtime_descriptor)), ("code_ref",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).code_ref).len()))])), ("created_at",mp::text(&(_field_0).created_at)?), ("updated_at",mp::text(&(_field_0).updated_at)?)]))]), Self::Search(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Search".to_owned())), ("0",{ if (_field_0).len() > 32 { return None; } serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("node",mp::object_value(&[("block_id",mp::text(&(&(item).node).block_id)?), ("workspace_id",mp::text(&(&(item).node).workspace_id)?), ("page_id",mp::text(&(&(item).node).page_id)?), ("tab_id",mp::text(&(&(item).node).tab_id)?), ("parent_block_id",match (&(&(item).node).parent_block_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("rank",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).node).rank).len()))])), ("presentation",match &(&(item).node).presentation {crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Page => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Drawer => mp::object_value(&[("variant",serde_json::Value::String("Drawer".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Modal => mp::object_value(&[("variant",serde_json::Value::String("Modal".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Inline => mp::object_value(&[("variant",serde_json::Value::String("Inline".to_owned()))])}), ("title",match (&(&(item).node).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("description",match (&(&(item).node).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("schema_version",serde_json::json!(*(&(&(item).node).schema_version))), ("created_at",mp::text(&(&(item).node).created_at)?), ("updated_at",mp::text(&(&(item).node).updated_at)?)])), ("ancestors",{ if (&(item).ancestors).len() > 32 { return None; } serde_json::Value::Array((&(item).ancestors).iter().map(|item| Some(mp::object_value(&[("block_id",mp::text(&(item).block_id)?), ("workspace_id",mp::text(&(item).workspace_id)?), ("page_id",mp::text(&(item).page_id)?), ("tab_id",mp::text(&(item).tab_id)?), ("parent_block_id",match (&(item).parent_block_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("rank",mp::object_value(&[("byte_count",serde_json::json!((&(item).rank).len()))])), ("presentation",match &(item).presentation {crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Page => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Drawer => mp::object_value(&[("variant",serde_json::Value::String("Drawer".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Modal => mp::object_value(&[("variant",serde_json::Value::String("Modal".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Inline => mp::object_value(&[("variant",serde_json::Value::String("Inline".to_owned()))])}), ("title",match (&(item).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("description",match (&(item).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("schema_version",serde_json::json!(*(&(item).schema_version))), ("created_at",mp::text(&(item).created_at)?), ("updated_at",mp::text(&(item).updated_at)?)]))).collect::<Option<Vec<_>>>()?) })]))).collect::<Option<Vec<_>>>()?) })]), Self::Summaries(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Summaries".to_owned())), ("0",{ if (_field_0).len() > 32 { return None; } serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("block_id",mp::text(&(item).block_id)?), ("workspace_id",mp::text(&(item).workspace_id)?), ("page_id",mp::text(&(item).page_id)?), ("tab_id",mp::text(&(item).tab_id)?), ("parent_block_id",match (&(item).parent_block_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("rank",mp::object_value(&[("byte_count",serde_json::json!((&(item).rank).len()))])), ("presentation",match &(item).presentation {crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Page => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Drawer => mp::object_value(&[("variant",serde_json::Value::String("Drawer".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Modal => mp::object_value(&[("variant",serde_json::Value::String("Modal".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Inline => mp::object_value(&[("variant",serde_json::Value::String("Inline".to_owned()))])}), ("title",match (&(item).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("description",match (&(item).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("schema_version",serde_json::json!(*(&(item).schema_version))), ("created_at",mp::text(&(item).created_at)?), ("updated_at",mp::text(&(item).updated_at)?)]))).collect::<Option<Vec<_>>>()?) })]), Self::Descendants(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Descendants".to_owned())), ("0",{ if (_field_0).len() > 32 { return None; } serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("node",mp::object_value(&[("block_id",mp::text(&(&(item).node).block_id)?), ("workspace_id",mp::text(&(&(item).node).workspace_id)?), ("page_id",mp::text(&(&(item).node).page_id)?), ("tab_id",mp::text(&(&(item).node).tab_id)?), ("parent_block_id",match (&(&(item).node).parent_block_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("rank",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).node).rank).len()))])), ("presentation",match &(&(item).node).presentation {crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Page => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Drawer => mp::object_value(&[("variant",serde_json::Value::String("Drawer".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Modal => mp::object_value(&[("variant",serde_json::Value::String("Modal".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Inline => mp::object_value(&[("variant",serde_json::Value::String("Inline".to_owned()))])}), ("title",match (&(&(item).node).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("description",match (&(&(item).node).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("schema_version",serde_json::json!(*(&(&(item).node).schema_version))), ("created_at",mp::text(&(&(item).node).created_at)?), ("updated_at",mp::text(&(&(item).node).updated_at)?)])), ("depth",serde_json::json!(*(&(item).depth))), ("has_children",serde_json::Value::Bool(*(&(item).has_children))), ("path",mp::object_value(&[("item_count",serde_json::json!((&(item).path).len()))]))]))).collect::<Option<Vec<_>>>()?) })]), Self::DeleteImpact(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("DeleteImpact".to_owned())), ("0",mp::object_value(&[("affected_count",serde_json::json!(*(&(_field_0).affected_count)))]))]), Self::DeleteSubtree(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("DeleteSubtree".to_owned())), ("0",mp::object_value(&[("deleted_count",serde_json::json!(*(&(_field_0).deleted_count)))]))]), Self::Code(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Code".to_owned())), ("0",mp::object_value(&[("block_id",mp::text(&(_field_0).block_id)?), ("page_id",mp::text(&(_field_0).page_id)?), ("source_code",mp::text(&(_field_0).source_code)?), ("source_sha256",match (&(_field_0).source_sha256).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]), Self::Fragment(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Fragment".to_owned())), ("0",mp::object_value(&[("block_id",mp::text(&(_field_0).block_id)?), ("page_id",mp::text(&(_field_0).page_id)?), ("source_revision",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).source_revision).len()))])), ("source_fragment",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).source_fragment).len()))])), ("start_line",serde_json::json!(*(&(_field_0).start_line))), ("start_column",serde_json::json!(*(&(_field_0).start_column))), ("end_line",serde_json::json!(*(&(_field_0).end_line))), ("end_column",serde_json::json!(*(&(_field_0).end_column))), ("total_lines",serde_json::json!(*(&(_field_0).total_lines))), ("total_chars",serde_json::json!(*(&(_field_0).total_chars))), ("next_line",match (&(_field_0).next_line).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("next_column",match (&(_field_0).next_column).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("truncated_by_max_chars",serde_json::Value::Bool(*(&(_field_0).truncated_by_max_chars)))]))]), Self::RuntimeAssembly(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("RuntimeAssembly".to_owned())), ("0",mp::object_value(&[("layers",{ if (&(_field_0).layers).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).layers).iter().map(|item| Some(mp::object_value(&[("block_id",mp::text(&(item).block_id)?), ("tab_id",mp::text(&(item).tab_id)?), ("parent_block_id",match (&(item).parent_block_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("title",match (&(item).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("presentation",match &(item).presentation {crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Page => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Drawer => mp::object_value(&[("variant",serde_json::Value::String("Drawer".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Modal => mp::object_value(&[("variant",serde_json::Value::String("Modal".to_owned()))]), crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto::Inline => mp::object_value(&[("variant",serde_json::Value::String("Inline".to_owned()))])}), ("schema_version",serde_json::json!(*(&(item).schema_version))), ("input_mapping",mp::object_value(&[("item_count",serde_json::json!((&(item).input_mapping).len()))])), ("output_mapping",mp::object_value(&[("item_count",serde_json::json!((&(item).output_mapping).len()))])), ("runtime_descriptor",mp::json_summary(&(item).runtime_descriptor)), ("code_ref",mp::object_value(&[("byte_count",serde_json::json!((&(item).code_ref).len()))])), ("source_revision",match (&(item).source_revision).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) })]))]), Self::NoContent => mp::object_value(&[("variant",serde_json::Value::String("NoContent".to_owned()))])})
    }

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
