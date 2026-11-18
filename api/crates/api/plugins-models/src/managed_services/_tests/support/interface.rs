use super::*;

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl ManagedServiceInput {
    pub fn test_new(
        event_policy: Arc<crate::managed_services::event_projection::OperationPolicy>,
        interface_id: String,
        payload: serde_json::Value,
        stream: Option<crate::managed_services::stream::StreamInput>,
    ) -> Self {
        Self {
            event_policy,
            interface_id,
            payload,
            stream,
        }
    }
}
