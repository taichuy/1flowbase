use super::SnapshotLifetime;

impl SnapshotLifetime {
    #[doc(hidden)]
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_reference_count_for_test_support(&self, references: usize) {
        self.0.lock().unwrap().references = references;
    }
}
