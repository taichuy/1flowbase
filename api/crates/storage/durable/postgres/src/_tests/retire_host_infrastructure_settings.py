"""Exercise the real retirement migration in an isolated PostgreSQL schema.

Run with API_DATABASE_URL set. Every fixture and DDL change is rolled back.
"""
import itertools
import os
from pathlib import Path
import subprocess
import unittest
import uuid


MIGRATION = (Path(__file__).parents[2] / 'migrations' /
             '20260922143000_retire_host_infrastructure_settings.sql').read_text()
CACHE = ['get_host_infrastructure_cache_overview', 'list_host_infrastructure_cache_entries',
         'reveal_host_infrastructure_cache_entry', 'clear_host_infrastructure_cache_entry',
         'clear_host_infrastructure_cache_domain']
MEMORY = ['get_host_infrastructure_memory_overview', 'get_host_infrastructure_memory_stats_overview',
          'get_host_infrastructure_memory_stats', 'list_host_infrastructure_memory_entries',
          'list_host_infrastructure_memory_tree', 'search_host_infrastructure_memory_entries',
          'reveal_host_infrastructure_memory_entry']


def sql_string(value):
    return "'" + str(value).replace("'", "''") + "'"


def exercise(migration):
    schema = 'retire_infra_' + uuid.uuid4().hex
    sql = ['begin;', f'create schema {schema};', f'set local search_path = {schema},public;']
    for table in ['role_console_group_policies', 'role_console_operation_policies',
                  'host_infrastructure_provider_configs', 'workspace_console_settings_order_items']:
        sql.append(f'create table {schema}.{table} (like public.{table} including all);')
    # LIKE copies the actual installed constraints and indexes; explicit FKs below
    # reproduce policy cascade behavior without referencing shared users/roles.
    sql.append('alter table role_console_operation_policies add foreign key (group_policy_id,role_id) '
               'references role_console_group_policies(id,role_id) on delete cascade;')
    states = [None, (False, 'full'), (True, 'full'), (False, 'custom'), (True, 'custom')]
    expected = []
    for cache_state, memory_state in itertools.product(states, repeat=2):
        role = uuid.uuid4()
        for state, group, operations in [(cache_state, 'system.host-infrastructure', CACHE),
                                         (memory_state, 'system.memory-observation', MEMORY)]:
            if state is None:
                continue
            enabled, strategy = state
            policy = uuid.uuid4()
            sql.append(f"insert into role_console_group_policies (id,role_id,group_kind,group_id,enabled,strategy) "
                       f"values ('{policy}','{role}','settings_feature','{group}',{str(enabled).lower()},'{strategy}');")
            # Store alternating true/false custom values even under disabled/full.
            for index, operation in enumerate(operations):
                selected = index % 2 == 0
                sql.append(f"insert into role_console_operation_policies (id,role_id,group_policy_id,operation_id,policy_kind,simple_enabled) "
                           f"values ('{uuid.uuid4()}','{role}','{policy}','{operation}','simple',{str(selected).lower()});")
                expected.append((role, operation, enabled and (strategy == 'full' or selected)))
        if cache_state is not None or memory_state is not None:
            for state, operations in [(cache_state, CACHE), (memory_state, MEMORY)]:
                if state is None:
                    expected.extend((role, operation, False) for operation in operations)
    # Non-empty historical configuration must survive retirement verbatim.
    sql.append(f"insert into host_infrastructure_provider_configs "
               f"(id,installation_id,extension_id,provider_code,config_ref,enabled_contracts,config_json,status,updated_by) "
               f"values ('{uuid.uuid4()}','{uuid.uuid4()}','test.host','test','secret://system/test',"
               f"ARRAY['cache-store'],'{{\"host\":\"historical\"}}','pending_restart','{uuid.uuid4()}');")
    sql.append(migration)
    for role, operation, allowed in expected:
        sql.append(f"do $$ begin if (select g.enabled and (g.strategy='full' or o.simple_enabled) from role_console_operation_policies o "
                   f"join role_console_group_policies g on g.id=o.group_policy_id "
                   f"where o.role_id='{role}' and o.operation_id='{operation}' and g.group_id='system.memory-observation') "
                   f"is distinct from {str(allowed).lower()} then raise exception 'authorization changed for {operation}'; end if; end $$;")
    sql.append("do $$ begin if exists(select 1 from role_console_group_policies where group_id='system.host-infrastructure') "
               "then raise exception 'retired group remains'; end if; "
               "if not exists(select 1 from retired_host_infrastructure_settings where source_table='host_infrastructure_provider_configs' "
               "and record->'config_json'->>'host'='historical') then raise exception 'configuration lost'; end if; end $$;")
    sql.append('rollback;')
    return subprocess.run(['psql', os.environ['API_DATABASE_URL'], '-X', '-q', '-v', 'ON_ERROR_STOP=1'],
                          input='\n'.join(sql), text=True, capture_output=True, check=False)


class RetirementMigrationTest(unittest.TestCase):
    def test_preserves_effective_rights_for_all_25_group_combinations_and_archives_data(self):
        result = exercise(MIGRATION)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_detects_full_grant_expansion(self):
        mutated = MIGRATION.replace("set enabled = excluded.enabled, strategy = 'custom'",
                                     "set enabled = excluded.enabled, strategy = 'full'")
        result = exercise(mutated)
        # Effective authorization uses group strategy, so a full target is forbidden.
        self.assertNotEqual(mutated, MIGRATION)
        # Explicit anti-expansion check also protects new roles without memory groups.
        # The behavioral assertion above must evaluate full strategies as the API does.
        self.assertNotEqual(result.returncode, 0, 'mutation escaped authorization checks')


if __name__ == '__main__':
    unittest.main()
