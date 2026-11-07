use super::*;
use crate::event_center::api_projection::{self as ep, ApiEventInput, ApiEventOperationContext};
use serde_json::json;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(
    super::list_pricing_rules,
    super::create_pricing_rule,
    super::update_pricing_rule,
    super::delete_pricing_rule,
    super::get_pricing_catalog,
    super::import_pricing_catalog,
    super::sync_pricing_catalog,
    super::list_credit_accounts,
    super::get_credit_account,
    super::list_credit_ledger,
    super::grant_credit,
    super::charge_credit,
    super::adjust_credit,
    super::enable_charge,
    super::disable_charge,
    super::refund_credit
))]
struct BillingApiDocs;

fn at() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
        .to_offset(time::UtcOffset::from_hms(-5, -30, -45).unwrap())
        .replace_nanosecond(123_456_789)
        .unwrap()
}

fn assert_wire_schema<T: ToSchema>(actual: &impl Serialize) -> Value {
    let wire = serde_json::to_value(actual).unwrap();
    let schema = ep::schema::<T>();
    // Exact owner-field inventory detects drift in schema-only external DTOs,
    // including additions that a permissive object validator would otherwise allow.
    assert_eq!(
        wire.as_object().unwrap().keys().collect::<Vec<_>>(),
        schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>()
    );
    assert!(jsonschema::validator_for(&schema).unwrap().is_valid(&wire));
    wire
}

#[test]
fn billing_openapi_declares_all_existing_operation_identities_and_query_locations() {
    let spec = serde_json::to_value(BillingApiDocs::openapi()).unwrap();
    let base = "/api/console/settings/billing";
    let expected = [
        ("get", "/pricing-rules", "billing.pricing_rules.list"),
        ("post", "/pricing-rules", "billing.pricing_rules.create"),
        (
            "patch",
            "/pricing-rules/{id}",
            "billing.pricing_rules.update",
        ),
        (
            "delete",
            "/pricing-rules/{id}",
            "billing.pricing_rules.delete",
        ),
        ("get", "/pricing-catalog", "billing.pricing_catalog.view"),
        (
            "post",
            "/pricing-catalog/import",
            "billing.pricing_catalog.import",
        ),
        (
            "post",
            "/pricing-catalog/sync",
            "billing.pricing_catalog.sync",
        ),
        ("get", "/credit-accounts", "billing.credit_accounts.list"),
        (
            "get",
            "/credit-accounts/{user_id}",
            "billing.credit_accounts.view",
        ),
        ("get", "/credit-ledger", "billing.credit_ledger.list"),
        ("post", "/credits/{user_id}/grant", "billing.credit.grant"),
        ("post", "/credits/{user_id}/charge", "billing.credit.charge"),
        ("post", "/credits/{user_id}/adjust", "billing.credit.adjust"),
        ("post", "/credits/{user_id}/enable", "billing.credit.enable"),
        (
            "post",
            "/credits/{user_id}/disable",
            "billing.credit.disable",
        ),
        ("post", "/credits/{user_id}/refund", "billing.credit.refund"),
    ];
    let actual_count = spec["paths"]
        .as_object()
        .unwrap()
        .values()
        .map(|path| {
            path.as_object()
                .unwrap()
                .keys()
                .filter(|method| {
                    matches!(
                        method.as_str(),
                        "get" | "post" | "patch" | "delete" | "put" | "head" | "options" | "trace"
                    )
                })
                .count()
        })
        .sum::<usize>();
    assert_eq!(actual_count, expected.len());
    for (method, suffix, identity) in expected {
        let operation = &spec["paths"][format!("{base}{suffix}")][method];
        assert_eq!(operation["operationId"], identity);
        assert!(operation["summary"].is_string());
        assert!(operation["description"].is_string());
        assert!(operation["responses"]["200"]["content"]["application/json"]["schema"].is_object());
    }
    for suffix in [
        "/pricing-rules",
        "/pricing-catalog",
        "/credit-accounts",
        "/credit-ledger",
    ] {
        let operation = &spec["paths"][format!("{base}{suffix}")]["get"];
        assert!(operation.get("requestBody").is_none());
        assert!(operation["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .all(|parameter| parameter["in"] == "query"));
    }
    let sync = &spec["paths"][format!("{base}/pricing-catalog/sync")]["post"];
    assert!(sync.get("requestBody").is_none());
}

#[test]
fn billing_credit_document_schemas_match_real_record_and_summary_serialization() {
    let user = Uuid::now_v7();
    let workspace = Uuid::now_v7();
    let account = control_plane::ports::CreditAccountRecord {
        id: Uuid::now_v7(),
        workspace_id: workspace,
        user_id: user,
        credit_unit: "USD".into(),
        charge_enabled: true,
        current_balance: "12.50".into(),
        reserved_amount: "2.00".into(),
        available_balance: "10.50".into(),
        credit_insufficient: false,
        revision: 7,
        created_at: at(),
        updated_at: at(),
    };
    let account_wire = assert_wire_schema::<BillingCreditAccountResponse>(&account);
    assert_eq!(
        account_wire["created_at"],
        json!([1969, 365, 18, 29, 15, 123456789, -5, -30, -45])
    );
    let validator =
        jsonschema::validator_for(&ep::schema::<BillingCreditAccountResponse>()).unwrap();
    let mut wrong = account_wire.clone();
    wrong["created_at"] = json!("1970-01-01T00:00:00Z");
    assert!(!validator.is_valid(&wrong));
    wrong["created_at"] = json!([1970, 1, 0, 0, 0, 0, 0, 0]);
    assert!(!validator.is_valid(&wrong));
    wrong = account_wire.clone();
    wrong["created_at"][8] = json!("-45");
    assert!(!validator.is_valid(&wrong));

    let transaction = control_plane::ports::CreditTransactionRecord {
        id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        account_id: account.id,
        workspace_id: workspace,
        user_id: user,
        billing_session_id: Some(Uuid::now_v7()),
        actor_user_id: Some(user),
        actor_plugin_id: None,
        transaction_type: "grant".into(),
        amount: "12.50".into(),
        balance_after: "12.50".into(),
        reserved_after: "2.00".into(),
        credit_unit: "USD".into(),
        reason: "Actual business grant".into(),
        source_type: Some("manual".into()),
        source_id: Some("business-source".into()),
        idempotency_key: "business-operation".into(),
        status: "completed".into(),
        metadata: json!({"business":{"rows":[1,2,3]}}),
        created_at: at(),
    };
    assert_wire_schema::<BillingCreditTransactionResponse>(&transaction);
    assert_wire_schema::<BillingPricingCatalogImportResponse>(
        &crate::model_pricing_catalog::PricingRuleInstallSummary {
            inserted: 1,
            skipped: 2,
            updated: 3,
            deleted: 4,
        },
    );
    assert_wire_schema::<BillingPricingCatalogSyncResponse>(
        &control_plane::ports::PricingCatalogSyncSummary {
            inserted: 1,
            updated: 2,
            unchanged: 3,
            retired: 4,
        },
    );
    assert_wire_schema::<BillingPricingRuleDeleteResponse>(&json!({"deleted":true}));
}

#[test]
fn billing_pricing_documents_preserve_rfc3339_fields_and_existing_tuple_timestamps() {
    let body = crate::model_pricing_catalog::builtin_pricing_rules()
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let body_wire = assert_wire_schema::<PricingRuleBody>(&body);
    assert!(body_wire["effective_from"].is_string());
    let rule = body_to_rule(body, Uuid::now_v7(), None).unwrap();
    let response = PricingRuleResponse::from(rule);
    let wire = assert_wire_schema::<PricingRuleResponse>(&response);
    assert!(wire["effective_from"].is_string());
    assert_eq!(wire["created_at"].as_array().unwrap().len(), 9);
    assert_eq!(wire["updated_at"].as_array().unwrap().len(), 9);
}

#[test]
fn billing_query_event_date_uses_the_real_serde_tuple_and_matches_its_document_schema() {
    use crate::routes::settings::billing_interface::BillingInput;
    let input = BillingInput::ListCreditLedger(PageQuery {
        limit: Some(10),
        offset: None,
        user_id: Some(Uuid::now_v7()),
        before_created_at: Some(at()),
        before_id: Some(Uuid::now_v7()),
    });
    let document = input
        .project_document_input(&ApiEventOperationContext {
            interface_id: "billing.credit_ledger.list".into(),
            method: "GET".into(),
            path: "/api/console/settings/billing/credit-ledger".into(),
        })
        .unwrap();
    assert_eq!(
        document["query"]["before_created_at"],
        serde_json::to_value(at()).unwrap()
    );
    assert!(jsonschema::validator_for(&ep::schema::<PageQuery>())
        .unwrap()
        .is_valid(&document["query"]));
}
