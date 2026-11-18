use super::*;
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl ExternalCallbackCredential {
    pub fn test_new(
        state: Arc<ApiState>,
        headers: HeaderMap,
        jar: CookieJar,
        raw_query: Option<String>,
        admission: CallbackAdmission,
    ) -> Self {
        Self {
            state,
            headers,
            jar,
            raw_query,
            admission,
        }
    }
}
