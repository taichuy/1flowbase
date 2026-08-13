-- Existing rows remain explicitly legacy: no compiler result or lock is invented.
alter table frontstage_block_codes
  add column source_sha256 text,
  add column dependency_lock jsonb,
  add column tailwind_toolchain_lock jsonb,
  add column generated_css text,
  add column generated_css_sha256 text,
  add column compiler_identity jsonb;

alter table frontstage_block_codes
  add constraint frontstage_block_codes_executable_shape_check check (
    (source_sha256 is null
      and dependency_lock is null
      and tailwind_toolchain_lock is null
      and generated_css is null
      and generated_css_sha256 is null
      and compiler_identity is null)
    or
    (source_sha256 ~ '^[0-9a-f]{64}$'
      and jsonb_typeof(dependency_lock) = 'array'
      and jsonb_typeof(tailwind_toolchain_lock) = 'object'
      and generated_css is not null
      and generated_css_sha256 ~ '^[0-9a-f]{64}$'
      and jsonb_typeof(compiler_identity) = 'object')
  );
