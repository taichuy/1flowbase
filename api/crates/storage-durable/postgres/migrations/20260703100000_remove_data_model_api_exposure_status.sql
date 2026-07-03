alter table model_definitions
  drop constraint if exists model_definitions_api_exposure_status_check;

alter table model_definitions
  drop column if exists api_exposure_status;

alter table data_source_instances
  drop constraint if exists data_source_instances_default_api_exposure_status_check;

alter table data_source_instances
  drop column if exists default_api_exposure_status;

alter table main_source_defaults
  drop constraint if exists main_source_defaults_api_exposure_status_check;

alter table main_source_defaults
  drop column if exists default_api_exposure_status;
