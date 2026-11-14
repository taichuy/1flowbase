use super::*;

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl Output { pub fn test_new(value: Value) -> Self { Self(value) } }
