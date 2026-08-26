alter table run_archive_upload_sessions add column if not exists created_by uuid;
alter table run_archive_upload_sessions add column if not exists updated_by uuid;
update run_archive_upload_sessions
   set created_by = coalesce(created_by, actor_user_id),
       updated_by = coalesce(updated_by, actor_user_id)
 where created_by is null
    or updated_by is null;
alter table run_archive_upload_sessions alter column created_by set not null;
alter table run_archive_upload_sessions alter column updated_by set not null;
create index if not exists run_archive_upload_sessions_scope_created_id_idx
    on run_archive_upload_sessions (scope_id, created_at, id);

alter table run_archive_upload_chunks add column if not exists id uuid;
alter table run_archive_upload_chunks add column if not exists scope_id uuid;
alter table run_archive_upload_chunks add column if not exists updated_at timestamptz;
alter table run_archive_upload_chunks add column if not exists created_by uuid;
alter table run_archive_upload_chunks add column if not exists updated_by uuid;
update run_archive_upload_chunks chunks
   set id = coalesce(
           chunks.id,
           md5(
               '1flowbase.run_archive_upload_chunk:'
               || chunks.session_id::text
               || ':'
               || chunks.chunk_index::text
           )::uuid
       ),
       scope_id = sessions.scope_id,
       updated_at = coalesce(chunks.updated_at, chunks.created_at),
       created_by = coalesce(chunks.created_by, sessions.actor_user_id),
       updated_by = coalesce(chunks.updated_by, sessions.actor_user_id)
  from run_archive_upload_sessions sessions
 where chunks.session_id = sessions.id
   and (
       chunks.id is null
       or chunks.scope_id is null
       or chunks.updated_at is null
       or chunks.created_by is null
       or chunks.updated_by is null
   );
alter table run_archive_upload_chunks alter column id set not null;
alter table run_archive_upload_chunks alter column scope_id set not null;
alter table run_archive_upload_chunks alter column updated_at set default now();
alter table run_archive_upload_chunks alter column updated_at set not null;
alter table run_archive_upload_chunks alter column created_by set not null;
alter table run_archive_upload_chunks alter column updated_by set not null;
create index if not exists run_archive_upload_chunks_scope_created_id_idx
    on run_archive_upload_chunks (scope_id, created_at, id);

alter table run_archive_import_jobs add column if not exists created_by uuid;
alter table run_archive_import_jobs add column if not exists updated_by uuid;
update run_archive_import_jobs
   set created_by = coalesce(created_by, actor_user_id),
       updated_by = coalesce(updated_by, actor_user_id)
 where created_by is null
    or updated_by is null;
alter table run_archive_import_jobs alter column created_by set not null;
alter table run_archive_import_jobs alter column updated_by set not null;
create index if not exists run_archive_import_jobs_scope_created_id_idx
    on run_archive_import_jobs (scope_id, created_at, id);

alter table run_archive_import_mappings add column if not exists id uuid;
alter table run_archive_import_mappings add column if not exists scope_id uuid;
alter table run_archive_import_mappings add column if not exists updated_at timestamptz;
alter table run_archive_import_mappings add column if not exists created_by uuid;
alter table run_archive_import_mappings add column if not exists updated_by uuid;
update run_archive_import_mappings mappings
   set id = coalesce(
           mappings.id,
           md5(
               '1flowbase.run_archive_import_mapping:'
               || mappings.job_id::text
               || ':'
               || mappings.entity_kind
               || ':'
               || mappings.source_id
           )::uuid
       ),
       scope_id = jobs.scope_id,
       updated_at = coalesce(mappings.updated_at, mappings.created_at),
       created_by = coalesce(mappings.created_by, jobs.actor_user_id),
       updated_by = coalesce(mappings.updated_by, jobs.actor_user_id)
  from run_archive_import_jobs jobs
 where mappings.job_id = jobs.id
   and (
       mappings.id is null
       or mappings.scope_id is null
       or mappings.updated_at is null
       or mappings.created_by is null
       or mappings.updated_by is null
   );
alter table run_archive_import_mappings alter column id set not null;
alter table run_archive_import_mappings alter column scope_id set not null;
alter table run_archive_import_mappings alter column updated_at set default now();
alter table run_archive_import_mappings alter column updated_at set not null;
alter table run_archive_import_mappings alter column created_by set not null;
alter table run_archive_import_mappings alter column updated_by set not null;
create index if not exists run_archive_import_mappings_scope_created_id_idx
    on run_archive_import_mappings (scope_id, created_at, id);
