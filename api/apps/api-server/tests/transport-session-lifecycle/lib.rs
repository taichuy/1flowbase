#![allow(dead_code)]
use control_plane::ports::{ProviderRuntimeExecutionContext, ProviderRuntimeInvocationOutput};

#[path = "../../src/provider_runtime/transport_session_lifecycle.rs"]
mod transport_session_lifecycle;

// Adapter for one adjacent preexisting fixture; lifecycle behavior above is production source.
fn preserve_provider_invocation_outcome(
    invocation: anyhow::Result<ProviderRuntimeInvocationOutput>,
    completion: anyhow::Result<()>,
) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
    match (invocation, completion) {
        (Err(primary), _) => Err(primary),
        (Ok(_), Err(error)) => Err(error),
        (Ok(output), Ok(())) => Ok(output),
    }
}

#[cfg(test)]
mod provider_runtime {
    pub(crate) use crate::transport_session_lifecycle::TransportConnectionScopeGuard;
}
#[cfg(test)]
#[path = "../../src/routes/application_public_api/openai/http_delivery.rs"]
pub(crate) mod http_delivery;
#[cfg(test)]
mod routes {
    pub(crate) mod application_public_api {
        pub(crate) mod openai {
            pub(crate) use crate::http_delivery;
        }
    }
}

#[cfg(test)]
mod native_projection {
    include!(concat!(env!("OUT_DIR"), "/native_projection.rs"));
    mod tests {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/_tests/projection.rs"));
    }
}

// Existing control-plane test repository, compiled only in this isolated test host.
#[cfg(test)]
pub use control_plane::{errors, ports};
#[cfg(test)]
mod application_public_api {
    include!(concat!(env!("OUT_DIR"), "/application_fixtures.rs"));
}
