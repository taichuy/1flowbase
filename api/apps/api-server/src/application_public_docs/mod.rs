#[cfg(test)]
use api_public_gateway::application_public_docs::test_support::*;
pub use api_public_gateway::application_public_docs::*;
#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use utoipa::OpenApi;

    #[test]
    fn public_operation_registry_declares_unique_documented_operations() {
        let docs = DocTextResolver::new(DocsLocale::EnUs);
        let mut ids = HashSet::new();
        let mut routes = HashSet::new();

        for operation in PUBLIC_OPERATION_REGISTRY
            .iter()
            .chain(agent_logs::OPERATIONS)
        {
            assert!(
                ids.insert(operation.id_for_test_support()),
                "duplicate operation id"
            );
            assert!(
                routes.insert((
                    operation.method_for_test_support(),
                    operation.path_for_test_support()
                )),
                "duplicate public route"
            );
            assert!(
                category_label(operation.category_id_for_test_support(), DocsLocale::EnUs)
                    .is_some(),
                "unknown category {}",
                operation.category_id_for_test_support()
            );
            assert!(
                !operation_responses(operation, &docs)
                    .as_object()
                    .expect("operation responses should be an object")
                    .is_empty(),
                "operation {} must declare responses",
                operation.id_for_test_support()
            );
            if let Some(request_body) = operation_request_body(operation, &docs) {
                assert!(
                    request_body.as_object().is_some(),
                    "operation {} request body should be an object",
                    operation.id_for_test_support()
                );
            }
        }
    }

    #[test]
    fn public_operation_registry_matches_global_openapi_paths() {
        let spec = serde_json::to_value(crate::openapi::ApiDoc::openapi())
            .expect("global openapi should serialize");
        let paths = spec["paths"]
            .as_object()
            .expect("global openapi paths should be an object");

        for operation in PUBLIC_OPERATION_REGISTRY
            .iter()
            .chain(agent_logs::OPERATIONS)
        {
            let Some(path_item) = paths.get(operation.path_for_test_support()) else {
                panic!(
                    "global openapi missing path {}",
                    operation.path_for_test_support()
                );
            };
            let method = operation.method_for_test_support().to_ascii_lowercase();
            assert!(
                path_item.get(&method).is_some(),
                "global openapi missing {} {}",
                operation.method_for_test_support(),
                operation.path_for_test_support()
            );
        }
    }
}
