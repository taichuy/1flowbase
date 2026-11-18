// Preserve each original item's visibility outside the explicit test-support boundary.
#[cfg(any(test, feature = "test-support"))]
macro_rules! test_support_item {
    ($production:vis, [$($attributes:tt)*] $($item:tt)*) => {
        $($attributes)*
        #[doc(hidden)]
        pub $($item)*
    };
}
#[cfg(not(any(test, feature = "test-support")))]
macro_rules! test_support_item {
    ($production:vis, [$($attributes:tt)*] $($item:tt)*) => {
        $($attributes)*
        $production $($item)*
    };
}
