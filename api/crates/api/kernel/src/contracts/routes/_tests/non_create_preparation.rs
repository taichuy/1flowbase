use super::*;
use crate::openapi_interface::contract::{ApiInput, ApiOperationContext, ApiOutput};
use serde_json::{json, Value};

struct ValueContract;
impl InterfaceContract for ValueContract {
    const CONTRACT_ID: &'static str = "non-create-preparation-fixture";
    const CONTRACT_VERSION: &'static str = "1";
}
impl ApiInput for ValueContract {
    fn project_document_input(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({}))
    }
}
impl ApiOutput for ValueContract {
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({}))
    }
}
struct Identity;
impl ConsoleInterfacePort<ValueContract, ValueContract> for Identity {
    fn execute<'a>(
        &'a self,
        _: &'a UserPrincipal,
        input: ValueContract,
    ) -> ConsoleInterfaceFuture<'a, ValueContract> {
        Box::pin(async move { Ok(input) })
    }
}
#[test]
fn non_create_preparation_accepts_application_and_missing_plans_but_rejects_create() {
    let binding = BindingId::new("http.non-create.fixture.v1").unwrap();
    for operation in ["applications.list", "model_definitions.create"] {
        let registry = compile_registry(
            "api-server.non-create-fixture",
            "api-server.non-create-fixture.graph.v1",
            &[ConsoleInterfaceDeclaration {
                interface_id: operation,
                binding_id: binding.as_str(),
                method: "POST",
                path: "/api/console/fixture",
                mutating: true,
            }],
            Arc::new(Identity),
        )
        .unwrap();
        let result = reject_create_plan(&registry, &binding);
        if operation == "model_definitions.create" {
            assert_eq!(
                result.unwrap_err().0.to_string(),
                "Create binding requires its typed Create input"
            );
        } else {
            assert!(result.is_ok());
        }
        assert!(reject_create_plan(
            &registry,
            &BindingId::new("http.missing.fixture.v1").unwrap()
        )
        .is_ok());
    }
}
