use plugin_framework::{HostContractCode, RuntimeSlotCode, StorageImplementationKind};

#[test]
fn host_contract_codes_are_stable() {
    assert_eq!(HostContractCode::StorageDurable.as_str(), "storage-durable");
    assert_eq!(
        HostContractCode::StorageEphemeral.as_str(),
        "storage-ephemeral"
    );
    assert_eq!(HostContractCode::StorageObject.as_str(), "storage-object");
    assert_eq!(HostContractCode::FileManagement.as_str(), "file_management");
    assert_eq!(HostContractCode::DataAccess.as_str(), "data_access");
}

#[test]
fn runtime_slot_codes_are_stable() {
    assert_eq!(RuntimeSlotCode::ModelProvider.as_str(), "model_provider");
    assert_eq!(RuntimeSlotCode::DataSource.as_str(), "data_source");
    assert_eq!(
        RuntimeSlotCode::DataImportSnapshot.as_str(),
        "data_import_snapshot"
    );
    assert_eq!(RuntimeSlotCode::FileProcessor.as_str(), "file_processor");
    assert_eq!(
        RuntimeSlotCode::NetworkEgressProvider.as_str(),
        "network_egress_provider"
    );
}

#[test]
fn storage_implementation_kinds_are_not_named_driver() {
    assert_eq!(
        StorageImplementationKind::Durable.as_str(),
        "storage-durable"
    );
    assert_eq!(
        StorageImplementationKind::Ephemeral.as_str(),
        "storage-ephemeral"
    );
    assert_eq!(StorageImplementationKind::Object.as_str(), "storage-object");
}
