alter table frontstage_block_codes
    drop constraint if exists frontstage_block_codes_executable_shape_check;

alter table frontstage_block_codes
    drop column if exists tailwind_toolchain_lock,
    drop column if exists generated_css,
    drop column if exists generated_css_sha256,
    drop column if exists compiler_identity;

drop table if exists frontstage_executable_upgrade_runs,
    frontstage_executable_upgrade_markers;
