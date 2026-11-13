pub(crate) struct TestResources {
    _database: postgres_test_support::PostgresTestSchema,
    filesystem_roots: Vec<std::path::PathBuf>,
}

impl TestResources {
    pub(crate) fn new(
        database: postgres_test_support::PostgresTestSchema,
        filesystem_roots: Vec<std::path::PathBuf>,
    ) -> Self {
        Self {
            _database: database,
            filesystem_roots,
        }
    }
}

impl Drop for TestResources {
    fn drop(&mut self) {
        for root in &self.filesystem_roots {
            if let Err(error) = std::fs::remove_dir_all(root) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    eprintln!(
                        "failed to clean API test filesystem root {}: {error}",
                        root.display()
                    );
                }
            }
        }
    }
}
