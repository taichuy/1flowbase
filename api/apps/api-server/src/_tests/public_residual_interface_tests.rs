use interface_runtime::{GraphFingerprint, RouteIdentity};

use crate::extension_bus::{production_interface_contributions, InterfaceContributionCollector};

#[test]
fn eil_f04_all_four_public_auth_routes_have_compiled_bindings() {
    let mut collector =
        InterfaceContributionCollector::new(GraphFingerprint::new("eil-f04-public-auth").unwrap());
    for contribution in production_interface_contributions() {
        collector.add(contribution).unwrap();
    }
    let registry = collector.compile(std::sync::Weak::new()).unwrap();

    for (method, path) in [
        ("GET", "/api/public/auth/providers"),
        ("GET", "/api/public/auth/login-instances"),
        ("POST", "/api/public/auth/sign-in"),
        ("POST", "/api/public/auth/sign-up"),
    ] {
        assert!(
            registry
                .binding_by_route(&RouteIdentity::new(method, path).unwrap())
                .is_some(),
            "missing compiled public binding for {method} {path}"
        );
    }
}
