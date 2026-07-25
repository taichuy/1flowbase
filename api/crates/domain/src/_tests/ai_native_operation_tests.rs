use serde_json::json;

use crate::{AiNativeCompactProfile, AiNativeGenerateProfile, AiNativeOperation};

#[test]
fn ai_native_operation_roundtrips_its_complete_kind_profile_matrix() {
    let fixtures = [
        (
            AiNativeOperation::generate(AiNativeGenerateProfile::Standard),
            json!({"kind": "generate", "profile": "standard"}),
        ),
        (
            AiNativeOperation::generate(AiNativeGenerateProfile::LocalSummary),
            json!({"kind": "generate", "profile": "local_summary"}),
        ),
        (
            AiNativeOperation::CountTokens,
            json!({"kind": "count_tokens", "profile": null}),
        ),
        (
            AiNativeOperation::compact(AiNativeCompactProfile::ResponsesCompact),
            json!({"kind": "compact", "profile": "responses_compact"}),
        ),
        (
            AiNativeOperation::compact(AiNativeCompactProfile::ResponsesCompactionV2),
            json!({"kind": "compact", "profile": "responses_compaction_v2"}),
        ),
    ];

    for (operation, envelope) in fixtures {
        assert_eq!(serde_json::to_value(operation).unwrap(), envelope);
        assert_eq!(
            serde_json::from_value::<AiNativeOperation>(envelope).unwrap(),
            operation
        );
    }
}

#[test]
fn ai_native_operation_rejects_unknown_kinds_and_profiles() {
    for envelope in [
        json!({"kind": "unknown", "profile": "standard"}),
        json!({"kind": "generate", "profile": "responses_compact"}),
        json!({"kind": "count_tokens", "profile": "standard"}),
        json!({"kind": "compact", "profile": "unknown"}),
    ] {
        assert!(serde_json::from_value::<AiNativeOperation>(envelope).is_err());
    }
}
