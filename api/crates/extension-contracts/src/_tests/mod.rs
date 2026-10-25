mod adapter_descriptor_tests;
mod authentication_contract_tests;
mod data_source_contract_tests;
#[path = "managed_contributions/lifecycle_contract_tests.rs"]
mod lifecycle_contract_tests;
#[path = "managed_contributions/managed_hook_contract_tests.rs"]
mod managed_hook_contract_tests;
mod plugin_data_contract_tests;
mod plugin_data_model_contract_tests;
mod provider_contract_tests;
mod runtime_target_tests;
mod semantic_terminal_tests;

#[path = "managed_contributions/managed_interface_contract_tests.rs"]
mod managed_interface_contract_tests;

#[path = "managed_contributions/managed_interface_reference_tests.rs"]
mod managed_interface_reference_tests;

mod plugin_credential_contract_tests;

mod provider_external_context_tests;

mod multiplex_duplex_tests;

mod notification_contract_tests;

#[path = "file_import/contract.rs"]
mod file_import_contract_tests;

#[path = "managed_contributions/managed_service_contract_tests.rs"]
mod managed_service_contract_tests;
