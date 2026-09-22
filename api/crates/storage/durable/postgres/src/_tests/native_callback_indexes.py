"""Verify the real migration and repository query on a rolled-back schema.

Run with API_DATABASE_URL set. No shared rows or indexes are changed.
"""
import json
import os
from pathlib import Path
import re
import subprocess
import unittest
from urllib.parse import unquote, urlsplit
import uuid


ROOT = Path(__file__).parents[2]
MIGRATION = (ROOT / 'migrations/20260922180000_native_callback_correlation_indexes.sql').read_text()
SOURCE = (ROOT / 'src/orchestration_runtime_repository/mod.rs').read_text()


def exercise(indexes):
    schema = 'native_callback_' + uuid.uuid4().hex
    body = SOURCE.split('async fn find_native_responses_callbacks_by_call_ids(', 1)[1]
    query = re.search(r'r#"(.*?)"#', body, re.S).group(1)
    # Exercise the exact repository predicate, including the coalesce recheck.
    predicate = query.split('where a.workspace_id = $1', 1)[1].split('order by', 1)[0]
    predicate = ('a.workspace_id = $1' + predicate)
    for key, value in [('$1', "'workspace'"), ('$2', "'app'"), ('$3', "'key'"),
                       ('$4', "'actor'"), ('$5', "ARRAY['wanted']")]:
        predicate = predicate.replace(key, value)
    sql = f"""
begin;
create schema {schema};
set local search_path = {schema},public;
create table applications(id text, workspace_id text);
create table flow_runs(id text, application_id text, api_key_id text, created_by text, run_mode text);
create table flow_run_callback_tasks(id integer, flow_run_id text, callback_kind text, request_payload jsonb);
insert into applications values ('app','workspace'),('foreign-app','foreign-workspace');
insert into flow_runs values ('run','app','key','actor','published_api_run'),
 ('foreign-key','app','other-key','actor','published_api_run'),
 ('foreign-actor','app','key','other-actor','published_api_run'),
 ('foreign-app','foreign-app','key','actor','published_api_run');
insert into flow_run_callback_tasks
select n,'run','llm_tool_calls',jsonb_build_object(
 'tool_calls',jsonb_build_array(jsonb_build_object('call_id','call_'||n,'arguments',repeat(md5(n::text),64))),
 'provider_metadata',jsonb_build_object('native_response',jsonb_build_object('response_id','resp_'||n)))
from generate_series(1,4000) n;
insert into flow_run_callback_tasks values
 (4001,'run','llm_tool_calls','{{"tool_calls":[{{"call_id":"wanted"}}],"provider_metadata":{{"native_response":{{"response_id":"needle"}}}}}}'),
 (4002,'run','llm_tool_calls','{{"tool_calls":[{{"id":"wanted"}}],"provider_metadata":{{"native_response":{{}}}}}}'),
 (4003,'run','llm_tool_calls','{{"tool_calls":[{{"call_id":"different","id":"wanted"}}],"provider_metadata":{{"native_response":{{}}}}}}'),
 (4004,'foreign-key','llm_tool_calls','{{"tool_calls":[{{"call_id":"wanted"}}],"provider_metadata":{{"native_response":{{}}}}}}'),
 (4005,'foreign-actor','llm_tool_calls','{{"tool_calls":[{{"call_id":"wanted"}}],"provider_metadata":{{"native_response":{{}}}}}}'),
 (4006,'foreign-app','llm_tool_calls','{{"tool_calls":[{{"call_id":"wanted"}}],"provider_metadata":{{"native_response":{{}}}}}}');
{MIGRATION if indexes else ''}
analyze flow_run_callback_tasks;
analyze flow_runs;
analyze applications;
select json_agg(c.id order by c.id) from flow_run_callback_tasks c
join flow_runs f on f.id=c.flow_run_id join applications a on a.id=f.application_id
where {predicate};
explain (format json) select c.id from flow_run_callback_tasks c
join flow_runs f on f.id=c.flow_run_id join applications a on a.id=f.application_id
where {predicate};
explain (format json) select id from flow_run_callback_tasks
where callback_kind='llm_tool_calls'
and request_payload #>> '{{provider_metadata,native_response,response_id}}'='needle';
rollback;
"""
    url = urlsplit(os.environ['API_DATABASE_URL'])
    env = {**os.environ, 'PGPASSWORD': unquote(url.password or '')}
    result = subprocess.run(['psql', '-X', '-qAt', '-h', url.hostname, '-p', str(url.port or 5432),
                             '-U', unquote(url.username), '-d', url.path[1:], '-v', 'ON_ERROR_STOP=1'],
                            input=sql, text=True, capture_output=True, env=env)
    if result.returncode:
        raise AssertionError(result.stderr)
    decoder = json.JSONDecoder()
    values, remaining = [], result.stdout.strip()
    while remaining:
        value, end = decoder.raw_decode(remaining)
        values.append(value)
        remaining = remaining[end:].strip()
    return values


class NativeCallbackIndexes(unittest.TestCase):
    def test_real_predicate_preserves_identity_and_uses_both_indexes(self):
        matches, calls, response = exercise(True)
        self.assertEqual(matches, [4001, 4002])
        self.assertIn('native_callback_call_identities', json.dumps(calls))
        self.assertIn('native_callback_response_identity', json.dumps(response))

    def test_missing_migration_preserves_results_but_loses_indexed_access(self):
        matches, calls, response = exercise(False)
        self.assertEqual(matches, [4001, 4002])
        self.assertNotIn('Index Name', json.dumps([calls, response]))


if __name__ == '__main__':
    unittest.main()
