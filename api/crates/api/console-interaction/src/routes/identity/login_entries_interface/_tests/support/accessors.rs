use super::*;
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl PublicLoginEntriesOutput { pub fn test_new(value: PublicLoginEntriesResponse) -> Self { Self(value) } }
