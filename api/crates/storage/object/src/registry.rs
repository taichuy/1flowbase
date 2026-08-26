use std::{collections::BTreeMap, sync::Arc};

use crate::{
    driver::FileStorageDriver,
    drivers::{local::LocalFileStorageDriver, rustfs::RustfsFileStorageDriver},
};

#[derive(Clone, Default)]
pub struct FileStorageDriverRegistry {
    drivers: BTreeMap<String, Arc<dyn FileStorageDriver>>,
}

impl FileStorageDriverRegistry {
    pub fn register(mut self, driver: Arc<dyn FileStorageDriver>) -> Self {
        self.drivers
            .insert(driver.driver_type().to_string(), driver);
        self
    }

    pub fn get(&self, driver_type: &str) -> Option<Arc<dyn FileStorageDriver>> {
        self.drivers.get(driver_type).cloned()
    }

    pub fn driver_types(&self) -> Vec<String> {
        self.drivers.keys().cloned().collect()
    }
}

pub fn builtin_driver_registry() -> FileStorageDriverRegistry {
    FileStorageDriverRegistry::default()
        .register(Arc::new(LocalFileStorageDriver))
        .register(Arc::new(RustfsFileStorageDriver))
}
