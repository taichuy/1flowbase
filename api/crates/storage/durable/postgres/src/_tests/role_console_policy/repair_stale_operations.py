"""Exercise the role policy repair against isolated PostgreSQL tables."""

import os
from pathlib import Path
import subprocess
import unittest
import uuid


MIGRATION = (Path(__file__).parents[3] / "migrations" /
             "20260925010000_repair_stale_role_console_operations.sql").read_text()
RENAMES = {
    "reveal_host_infrastructure_cache_entry": "host_infrastructure.cache.reveal",
    "clear_host_infrastructure_cache_entry": "host_infrastructure.cache.entry.clear",
    "clear_host_infrastructure_cache_domain": "host_infrastructure.cache.domain.clear",
    "reveal_host_infrastructure_memory_entry": "host_infrastructure.memory.reveal",
}


class RepairStaleOperationsTest(unittest.TestCase):
    def test_preserves_policy_values_and_prefers_existing_canonical_policy(self):
        schema = "repair_console_policy_" + uuid.uuid4().hex
        role = uuid.uuid4()
        group = uuid.uuid4()
        conflict_role = uuid.uuid4()
        sql = [
            "begin;",
            f"create schema {schema};",
            f"set local search_path = {schema}, public;",
            f"create table {schema}.role_console_operation_policies "
            "(like public.role_console_operation_policies including all);",
        ]
        for index, (old_id, new_id) in enumerate(RENAMES.items()):
            enabled = "true" if index % 2 == 0 else "false"
            sql.append(
                "insert into role_console_operation_policies "
                "(id,role_id,group_policy_id,operation_id,policy_kind,simple_enabled) "
                f"values ('{uuid.uuid4()}','{role}','{group}','{old_id}','simple',{enabled});"
            )
            sql.append(
                f"do $$ begin if exists (select 1 from role_console_operation_policies "
                f"where role_id='{role}' and operation_id='{new_id}') "
                "then raise exception 'fixture contains canonical policy'; end if; end $$;"
            )
        sql.extend([
            "insert into role_console_operation_policies "
            "(id,role_id,group_policy_id,operation_id,policy_kind,row_scope) "
            f"values ('{uuid.uuid4()}','{role}','{group}',"
            "'get_application_operation_bindings','row','own');",
            "insert into role_console_operation_policies "
            "(id,role_id,group_policy_id,operation_id,policy_kind,simple_enabled) values "
            f"('{uuid.uuid4()}','{conflict_role}','{group}',"
            "'reveal_host_infrastructure_cache_entry','simple',true),"
            f"('{uuid.uuid4()}','{conflict_role}','{group}',"
            "'host_infrastructure.cache.reveal','simple',false);",
            MIGRATION,
        ])
        for index, new_id in enumerate(RENAMES.values()):
            enabled = "true" if index % 2 == 0 else "false"
            sql.append(
                "do $$ begin if not exists (select 1 from role_console_operation_policies "
                f"where role_id='{role}' and operation_id='{new_id}' "
                f"and simple_enabled={enabled}) then raise exception "
                f"'policy value lost for {new_id}'; end if; end $$;"
            )
        sql.extend([
            "do $$ begin if exists (select 1 from role_console_operation_policies "
            "where operation_id in ("
            "'get_application_operation_bindings',"
            "'reveal_host_infrastructure_cache_entry',"
            "'clear_host_infrastructure_cache_entry',"
            "'clear_host_infrastructure_cache_domain',"
            "'reveal_host_infrastructure_memory_entry')) "
            "then raise exception 'stale operation remains'; end if; end $$;",
            "do $$ begin if not exists (select 1 from role_console_operation_policies "
            f"where role_id='{conflict_role}' "
            "and operation_id='host_infrastructure.cache.reveal' "
            "and simple_enabled=false) then raise exception "
            "'existing canonical policy was overwritten'; end if; end $$;",
            "rollback;",
        ])
        result = subprocess.run(
            ["psql", os.environ["API_DATABASE_URL"], "-X", "-q", "-v", "ON_ERROR_STOP=1"],
            input="\n".join(sql), text=True, capture_output=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
