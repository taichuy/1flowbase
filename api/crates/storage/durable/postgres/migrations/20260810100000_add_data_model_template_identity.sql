alter table model_definitions
  add column template_provider text,
  add column template_code text,
  add column template_version text;

update model_definitions
set template_provider = 'core',
    template_code = 'general',
    template_version = 'v1';

alter table model_definitions
  alter column template_provider set not null,
  alter column template_code set not null,
  alter column template_version set not null;
