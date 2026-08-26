alter table application_run_log_summaries
    add column if not exists api_key_name_snapshot text;

update application_run_log_summaries summaries
set api_key_name_snapshot = api_keys.name
from api_keys
where summaries.api_key_id = api_keys.id
  and summaries.api_key_name_snapshot is null;
