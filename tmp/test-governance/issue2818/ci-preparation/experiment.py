#!/usr/bin/env python3
"""Issue 2818 one-off controlled W. Not a product runner or natural PR latency."""
import argparse
from collections import Counter
import hashlib
import gzip
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

SEED_SHA = '7bb249e47d29dce87a291677884bd42962aa2046'
PRIOR_RUN = '42629370477'
PRIOR_NAME = 'issue2818-native31-resume-cache-42629370477-1'
PRIOR_ARTIFACT_ID = 13679813182
ARCHIVE_SHA = 'ff74478fc79a98b5d00d1e15657f266adfa49b57e80259e34c91ad3af7cbe15e'
ARCHIVE_BYTES = 1038687479
BASELINE = 'e3cdfefb7a5b85d1d7d9c2ef166e3b923c05de89'
W2_CANDIDATE = 'e0a2790bb08ec3b4c914b27e1bed9938c293d1fe'
W2_ALLOWED = ['.agents/skills/qa-evaluation/references/backend/console-settings-registration-gate.md', '.github/workflows/notification-subscription.yml', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/data_model_and_docs.rs', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/infrastructure_and_mcp.rs', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/plugins_and_models.rs', 'api/apps/api-server/src/routes/_tests/file_console_route_assembly_tests.rs', 'api/apps/api-server/src/routes/_tests/mod.rs', 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'api/apps/api-server/src/routes/console_route_assembly.rs', 'api/apps/api-server/src/routes/console_route_assembly/_tests/assembly.rs', 'api/apps/api-server/src/routes/mod.rs', 'api/apps/api-server/src/test_support/mod.rs', 'api/apps/api-server/tests/README.md', 'api/apps/api-server/tests/console_core_registry_tests.rs', 'scripts/node/agent-logs-exchange/ci-api.cjs', 'scripts/node/console-operation-registry-hygiene/README.md', 'scripts/node/console-operation-registry-hygiene/_tests/core.test.js', 'scripts/node/console-operation-registry-hygiene/compiled-assembly-checks.js']
W2_MAP = [{'assertion_source': 'api/apps/api-server/src/routes/console_route_assembly/_tests/assembly.rs', 'function': 'migrated_assembly_contains_every_console_router_owner_assembly', 'full_test_name': 'routes::console_route_assembly::tests::migrated_assembly_contains_every_console_router_owner_assembly', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/console_route_assembly/_tests/assembly.rs', 'function': 'complete_operation_specifications_match_the_production_route_set', 'full_test_name': 'routes::console_route_assembly::tests::complete_operation_specifications_match_the_production_route_set', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/console_route_assembly/_tests/assembly.rs', 'function': 'operation_specification_exact_set_reports_every_difference', 'full_test_name': 'routes::console_route_assembly::tests::operation_specification_exact_set_reports_every_difference', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'function': 'console_route_assembly_unclassified_route_fails_coverage', 'full_test_name': 'routes::_tests::console_route_assembly_tests::console_route_assembly_unclassified_route_fails_coverage', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'function': 'console_route_assembly_duplicate_ownership_fails_coverage', 'full_test_name': 'routes::_tests::console_route_assembly_tests::console_route_assembly_duplicate_ownership_fails_coverage', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'function': 'migrated_real_core_console_route_assembly_has_compiled_coverage', 'full_test_name': 'routes::_tests::console_route_assembly_tests::migrated_real_core_console_route_assembly_has_compiled_coverage', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'function': 'applications_routes_compile_exact_operations_and_resource_metadata', 'full_test_name': 'routes::_tests::console_route_assembly_tests::applications_routes_compile_exact_operations_and_resource_metadata', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'function': 'applications_closed_set_rejects_duplicate_or_missing_binding', 'full_test_name': 'routes::_tests::console_route_assembly_tests::applications_closed_set_rejects_duplicate_or_missing_binding', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'function': 'application_api_orchestration_runtime_routes_compile_exact_operations', 'full_test_name': 'routes::_tests::console_route_assembly_tests::application_api_orchestration_runtime_routes_compile_exact_operations', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/file_console_route_assembly_tests.rs', 'function': 'file_owner_routes_compile_from_exact_assemblies', 'full_test_name': 'routes::_tests::file_console_route_assembly_tests::file_owner_routes_compile_from_exact_assemblies', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/file_console_route_assembly_tests.rs', 'function': 'file_owner_routes_preserve_authenticated_data_acl_and_settings_operations', 'full_test_name': 'routes::_tests::file_console_route_assembly_tests::file_owner_routes_preserve_authenticated_data_acl_and_settings_operations', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/file_console_route_assembly_tests.rs', 'function': 'temporary_file_contracts_compile_without_projecting_credentials', 'full_test_name': 'routes::_tests::file_console_route_assembly_tests::temporary_file_contracts_compile_without_projecting_credentials', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_002_013_members_route_bindings_are_explicit_and_stable', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_members_route_bindings_are_explicit_and_stable', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_002_013_permissions_route_binding_is_explicit_and_stable', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_permissions_route_binding_is_explicit_and_stable', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_001_ui_management_route_bindings_are_explicit_and_granular', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_001_ui_management_route_bindings_are_explicit_and_granular', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_002_013_roles_route_bindings_are_explicit_and_stable', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_roles_route_bindings_are_explicit_and_stable', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_002_013_workspace_route_bindings_are_explicit_and_stable', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_workspace_route_bindings_are_explicit_and_stable', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_002_013_workspaces_route_binding_is_explicit_and_stable', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_workspaces_route_binding_is_explicit_and_stable', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_002_013_auth_center_route_bindings_are_explicit_and_stable', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_auth_center_route_bindings_are_explicit_and_stable', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_002_013_system_route_bindings_are_explicit_and_stable', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_system_route_bindings_are_explicit_and_stable', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'organization_console_route_assembly_keeps_each_operation_independent', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::organization_console_route_assembly_keeps_each_operation_independent', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/data_model_and_docs.rs', 'function': 'data_model_docs_and_data_source_routes_compile_exact_operations', 'full_test_name': 'routes::_tests::console_route_assembly_tests::data_model_and_docs::data_model_docs_and_data_source_routes_compile_exact_operations', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/infrastructure_and_mcp.rs', 'function': 'infrastructure_mcp_and_user_api_key_routes_compile_exact_operations', 'full_test_name': 'routes::_tests::console_route_assembly_tests::infrastructure_and_mcp::infrastructure_mcp_and_user_api_key_routes_compile_exact_operations', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/plugins_and_models.rs', 'function': 'ac_002_ac_013_plugins_and_models_owner_routes_have_explicit_assembly_ownership', 'full_test_name': 'routes::_tests::console_route_assembly_tests::plugins_and_models::ac_002_ac_013_plugins_and_models_owner_routes_have_explicit_assembly_ownership', 'old_target': 'console_core_registry_tests', 'new_target': 'api_private_routes', 'old_wrapper_attributes': ['#[test]'], 'new_direct_attributes_required': ['#[test]'], 'ignored': False}]
W2_NATIVE_SEVEN = ['ac_002_console_health_is_compiled_and_not_exposed_by_the_base_router', 'ac_003_unknown_core_operation_metadata_fails_instead_of_becoming_other', 'ac_003_every_console_route_is_owned_by_a_compiled_authorization_operation', 'ac_003_005_all_console_routes_have_distinct_static_english_interface_metadata', 'ac_009_core_compiled_catalog_resolves_every_active_display_reference_in_both_locales', 'ac_002_009_operation_semantics_are_explicit_before_legacy_mapping', 'ac_013_role_console_policy_workers_have_explicit_interface_metadata']
W2_LEAVES = ['api/apps/api-server/src/routes/console_route_assembly/_tests/assembly.rs', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'api/apps/api-server/src/routes/_tests/file_console_route_assembly_tests.rs', 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/data_model_and_docs.rs', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/infrastructure_and_mcp.rs', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/plugins_and_models.rs']
W2_PROTECTED = [{'path': 'api/apps/api-server/src/provider_runtime/mod.rs', 'sha256': '289ba1e169aa53e32a99f7a13b37acccfc5dd74a01f2d4214c16b00e6dbfa153'}, {'path': 'api/apps/api-server/src/app_state.rs', 'sha256': '9bec87713ad9dfd2a2ff0d30642e62fa4c2ae224228c4379481e5b8d4bb66223'}, {'path': 'api/apps/api-server/src/private_routes_test.rs', 'sha256': '1cf5415646e510d6baa266421da5e3225b45e01a272307fadf0cbfc57411b6da'}, {'path': 'api/apps/api-server/src/lib_root.rs', 'sha256': '6ef50240a154e61ff80fa49a22a6297afedb83b58b43e9093c34e773bf1572a6'}, {'path': 'api/apps/api-server/src/private_contracts_test.rs', 'sha256': '6ef50240a154e61ff80fa49a22a6297afedb83b58b43e9093c34e773bf1572a6'}, {'path': 'api/crates/api-test-selection/src/lib.rs', 'sha256': '40de6d57e108ffd0d2e3ee9b8e30bae3c068db5fd9eb1e978fb43c53d75ad9d5'}, {'path': 'api/apps/api-server/Cargo.toml', 'sha256': '93f182d7e43fdc68d27918a0e3f207305beb5deca70930565c0ce1a0b7243c9d'}, {'path': 'api/Cargo.toml', 'sha256': '1d4ead1d4c89915ca70271a02368ecdfb99b089c80865f1773fa93bc06919d57'}, {'path': 'api/apps/api-server/src/_tests/application/application_runtime_routes.rs', 'sha256': 'b596a6459b57bf9ae11e608a6848beebca0e32fd31e1b50f1374ae6cdd904114', 'allowed_change': 'none'}, {'path': 'api/apps/api-server/src/test_support/resources.rs', 'sha256': 'd46f990f5b78f4105268588f56ce5879641f7adb1e207c0df2e9d7f0956ba9d8', 'allowed_change': 'none'}, {'path': 'api/apps/api-server/src/_tests/support/mod.rs', 'sha256': '904ac356af6314e492adc70e00efe8ea35e9bc99626c696c23d56ff5546faf66', 'allowed_change': 'none'}, {'path': 'api/apps/api-server/src/_tests/support/applications.rs', 'sha256': 'e278f690953fcda9beb2e9be50f5ce4f8c10f25cb61b029da1c516462e0c3643', 'allowed_change': 'none'}, {'path': 'api/apps/api-server/src/_tests/support/auth.rs', 'sha256': '31e908761f95c280ecdc456d1c389c542964b0de98817984a8526ef393d05e33', 'allowed_change': 'none'}, {'path': 'api/apps/api-server/src/_tests/support/packages.rs', 'sha256': '663995938f7e4b98b40c07bfe75c1b8c210d8175713e27208da53faa33ff7ee8', 'allowed_change': 'none'}, {'path': 'api/apps/api-server/src/_tests/support/plugins.rs', 'sha256': '5a66ed399f1a042adcc7cd7d5313b61c993f5928c3d33588ee18a2f2fb5cc921', 'allowed_change': 'none'}]
W2_MAP_SHA = '85657d7abfb5c989dc6ee1245349a1b52891f96d2c838cfce80c0226133b0778'

HOME_ROOT = Path(__file__).resolve().parents[4]
SELF = Path(__file__).resolve()
EVIDENCE = HOME_ROOT / 'tmp/test-governance/issue2818/ci-evidence'
TEMP = Path(os.environ.get('RUNNER_TEMP', '/tmp')) / 'issue2818'
RUNNER = HOME_ROOT / 'scripts/shell/resource-job-run.py'
HEAVY = HOME_ROOT / 'scripts/shell/resource-heavy-run.py'
END = time.monotonic() + 70 * 60
SERIAL = 0
RESULTS = []
PG_GROUP = None
THREAD_STATE = {'next':0,'previous':{},'summaries':{}}

BUILDERS = r"""
const root=process.argv[1], path=require('node:path');
const runtime=process.env.ISSUE2818_FROZEN_RUNTIME ? JSON.parse(process.env.ISSUE2818_FROZEN_RUNTIME) : require(path.join(root,'scripts/node/testing/verify-runtime.js')).loadVerifyRuntimeConfig({repoRoot:root,env:process.env});
const backend=require(path.join(root,'scripts/node/verify/backend.js'));
const foundation=require(path.join(root,'scripts/node/foundation-contracts/core.js'));
const options={repoRoot:root,env:process.env,cargoJobs:runtime.backend.cargoJobs,cargoTestThreads:runtime.backend.cargoTestThreads,incremental:runtime.backend.incremental};
const seed=backend.buildBackendCommands({...options,target:'official-i18n-seed'});
const fast=['mcp-gateway','application-backend'].flatMap(id=>foundation.buildFoundationPlan({foundation:id}).packs[id].fast);
const checks=backend.buildBackendCommands({...options,target:'check',shard:'apps'});
process.stdout.write(JSON.stringify({runtime,tests:[...seed,...fast],checks}));
"""

def save(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + '\n')

def read(path):
    return json.loads(path.read_text())

def capture(argv, cwd=HOME_ROOT):
    return subprocess.check_output(argv, cwd=cwd, text=True, timeout=30).strip()

def stream_exec(argv, merged):
    # One kernel pipe preserves Cargo target headers before their child output.
    child = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    with Path(merged).open('wb') as combined:
        while True:
            chunk = os.read(child.stdout.fileno(), 65536)
            if not chunk:
                break
            combined.write(chunk)
            combined.flush()
            sys.stdout.buffer.write(chunk)
            sys.stdout.buffer.flush()
    return child.wait()

def rustc_thread_sample(processes, now):
    if now < THREAD_STATE['next']:
        return []
    previous=THREAD_STATE['previous']
    current={}
    snapshots=[]
    summaries={}
    ticks=os.sysconf('SC_CLK_TCK')
    for process in processes:
        if process['comm']!='rustc':
            continue
        rows=[]
        pid=process['pid']
        try:
            tasks=list((Path('/proc')/str(pid)/'task').iterdir())
        except FileNotFoundError:
            continue
        for task in tasks:
            try:
                stat=(task/'stat').read_text().rsplit(')',1)[1].split()
                user,system,start=int(stat[11]),int(stat[12]),int(stat[19])
                key=(pid,int(task.name),start)
                total=user+system
                old=previous.get(key)
                cpu=100*(total-old[1])/ticks/(now-old[0]) if old and now>old[0] else None
                current[key]=(now,total)
                rows.append({'tid':int(task.name),'name':(task/'comm').read_text().strip(),
                    'state':stat[0],'user_ticks':user,'system_ticks':system,'start_ticks':start,
                    'cpu_percent_since_previous':None if cpu is None else round(cpu,2)})
            except (FileNotFoundError,ProcessLookupError,ValueError,IndexError):
                pass
        snapshots.append({'pid':pid,'crate':process.get('crate_name'),'clock_ticks_per_second':ticks,
            'sampled_monotonic':now,'threads_reported_by_status':process['thread_count'],'threads':rows})
        summaries[pid]={'names':dict(Counter(r['name'] for r in rows)),
            'top_cpu':[{k:r[k] for k in ['tid','name','cpu_percent_since_previous']}
                for r in sorted(rows,key=lambda r:r['cpu_percent_since_previous'] or 0,reverse=True)[:4]]}
    THREAD_STATE.update(next=now+1,previous=current,summaries=summaries)
    return snapshots

def native_sample():
    group = Path('/sys/fs/cgroup') / os.environ['ONEFLOWBASE_TASK_BUDGET_CGROUP'].lstrip('/')
    stats = {k:int(v) for k,v in (line.split() for line in (group/'memory.stat').read_text().splitlines())}
    processes = []
    pids = set()
    for file in group.rglob('cgroup.procs'):
        try:
            pids.update(int(x) for x in file.read_text().split())
        except FileNotFoundError:
            pass
    for pid in pids:
        try:
            proc = Path('/proc') / str(pid)
            status = dict(line.split(':',1) for line in (proc/'status').read_text().splitlines() if ':' in line)
            row = {'pid':pid,'comm':status['Name'].strip(),
                   'rss':int(status.get('VmRSS','0 kB').split()[0])*1024,
                   'vm_hwm':int(status.get('VmHWM','0 kB').split()[0])*1024,
                   'thread_count':int(status.get('Threads','0').strip())}
            if row['comm'] in ('rustc','cargo'):
                argv = (proc/'cmdline').read_bytes().split(b'\0')
                def flag(key):
                    key = key.encode()
                    return argv[argv.index(key)+1].decode(errors='replace') if key in argv and argv.index(key)+1<len(argv) else None
                row['crate_name'] = flag('--crate-name')
                row['crate_type'] = flag('--crate-type')
                row['is_test'] = b'--test' in argv
            processes.append(row)
        except (FileNotFoundError,ProcessLookupError):
            pass
    now=time.monotonic()
    threads=rustc_thread_sample(processes,now)
    current = int((group/'memory.current').read_text())
    pg = int((PG_GROUP/'memory.current').read_text()) if PG_GROUP else 0
    return {'monotonic':time.monotonic(),'memory_current':current,'pg_memory_current':pg,
            'aggregate_current':current+pg,'memory_stat':stats,'processes':processes,
            'rustc_thread_samples':threads,'thread_sampling_interval_seconds':1,
            'tree_rss_sample':sum(r['rss'] for r in processes),
            'max_process_vm_hwm':max((r['vm_hwm'] for r in processes),default=0),
            'memory_pressure':(group/'memory.pressure').read_text().strip(),
            'cpu_stat':{k:int(v) for k,v in (line.split() for line in (group/'cpu.stat').read_text().splitlines())}}

def run(label, argv, cwd=HOME_ROOT, env=None, heavy=False, seconds=1200, expected=0):
    global SERIAL
    SERIAL += 1
    folder = EVIDENCE / f'{SERIAL:03d}-{label}'
    merged = EVIDENCE / f'{SERIAL:03d}-{label}.combined.log'
    limit = min(seconds, END-time.monotonic())
    if limit <= 0:
        raise RuntimeError('global controlled-experiment deadline exhausted')
    actual = ([sys.executable, str(HEAVY)] if heavy else []) + [
        sys.executable, str(SELF), 'exec', str(merged), *argv]
    wrapper = [sys.executable, str(RUNNER), '--deadline-seconds', str(limit), '--output-dir', str(folder), '--', *actual]
    started = time.monotonic()
    next_report = started
    print(json.dumps({'stage':label,'event':'started','deadline_seconds':round(limit,2)}),flush=True)
    child = subprocess.Popen(wrapper, cwd=cwd, env=env)
    peaks = {'tree_rss_sample':0,'max_process_vm_hwm':0,'aggregate_current':0}
    count = 0
    with (EVIDENCE/f'{SERIAL:03d}-{label}.native-samples.jsonl').open('w') as samples:
        while child.poll() is None:
            sample = native_sample()
            samples.write(json.dumps(sample)+'\n')
            count += 1
            for key in peaks:
                peaks[key] = max(peaks[key], sample[key])
            if time.monotonic() >= next_report:
                stat=sample['memory_stat']
                print(json.dumps({'stage':label,'elapsed_seconds':round(time.monotonic()-started,1),
                    'memory_current':sample['memory_current'],'tree_rss_sample':sample['tree_rss_sample'],
                    'anon':stat.get('anon'),'file':stat.get('file'),'pgscan':stat.get('pgscan'),
                    'refault_file':stat.get('workingset_refault_file'),
                    'pressure':sample['memory_pressure'],
                    'rustc':[{'crate':r.get('crate_name'),'type':r.get('crate_type'),
                              'test':r.get('is_test'),'rss':r['rss'],'hwm':r['vm_hwm'],
                              'thread_count':r['thread_count'],
                              'threads':THREAD_STATE['summaries'].get(r['pid'],{})}
                             for r in sample['processes'] if r['comm']=='rustc'][:4]}),flush=True)
                next_report=time.monotonic()+30
            time.sleep(.1)
    code = child.wait()
    print(json.dumps({'stage':label,'event':'finished','elapsed_seconds':round(time.monotonic()-started,2),
                      'exit_code':code}),flush=True)
    save(EVIDENCE/f'{SERIAL:03d}-{label}.sample-peaks.json',
         {'quality':'100ms samples; not exact simultaneous kernel peak','samples':count,**peaks})
    receipt = read(folder/'finished.json')
    row = {'label':label,'argv':argv,'cwd':str(cwd),'exit_code':code,
           'wall_seconds':time.monotonic()-started,'receipt':str(folder/'finished.json'),
           'merged_log':str(merged),'stdout_log':str(folder/'stdout.log'),
           'stderr_log':str(folder/'stderr.log'),'classification':receipt['classification']}
    RESULTS.append(row)
    counts=re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;',merged.read_text(errors='replace') if merged.exists() else '')
    summary={'source_sha':os.environ.get('ISSUE2818_OBSERVED_SOURCE_SHA',os.environ.get('CANDIDATE_SHA')),'requested_candidate':os.environ.get('CANDIDATE_SHA'),'phase':label,'exit':code,
        'classification':receipt['classification'],'elapsed_seconds':receipt['elapsed_seconds'],
        'scope_peak_bytes':receipt['resources'].get('fields',{}).get('memory.peak',{}).get('value'),
        'task_sample_peak_bytes':peaks['aggregate_current'],'oom':bool(receipt['oom_evidence']),
        'cleanup_complete':receipt['cleanup']['complete'],
        'actual_tests':None if not counts else {'passed':sum(int(r[0]) for r in counts),
            'failed':sum(int(r[1]) for r in counts),'ignored':sum(int(r[2]) for r in counts)}}
    save(EVIDENCE/f'{SERIAL:03d}-{label}.summary.json',summary)
    print(json.dumps({'ci_phase_summary':summary}),flush=True)
    save(EVIDENCE/'commands.json', RESULTS)
    if not receipt['cleanup']['complete']:
        raise RuntimeError(f'{label}: incomplete owned-process cleanup')
    if receipt['classification'] != 'exit':
        raise RuntimeError(f'{label}: {receipt["classification"]}; no unchanged retry')
    if expected == 'optional':
        return row
    if expected == 'nonzero':
        if code == 0:
            raise RuntimeError(f'{label}: injected invalid source was accepted')
    elif code != expected:
        raise RuntimeError(f'{label}: command exit {code}; see raw logs')
    return row

def budget_snapshot():
    group = Path('/sys/fs/cgroup') / os.environ['ONEFLOWBASE_TASK_BUDGET_CGROUP'].lstrip('/')
    names = ['memory.max','memory.high','memory.swap.max','memory.current',
             'memory.peak','memory.events','cpu.max','cpu.stat','cgroup.events']
    data = {name:(group/name).read_text().strip() for name in names}
    data.update(inode=group.stat().st_ino, path=str(group))
    return data

def plan(root):
    data = json.loads(capture(['node','-e',BUILDERS,str(root)],root))
    assert len(data['tests']) == 4, 'frozen W requires exactly the original four filter commands'
    expected = ['ac_002_decodes_digest_verified_build_time_official_seed',
                '_tests::dynamic_backend_consumer_inventory::',
                'mcp_protocol_routes','model_definition_routes_']
    assert all(q in command['args'] for q,command in zip(expected,data['tests']))
    assert all(command['command']=='cargo' and command['args'][0]=='test'
               for command in data['tests'])
    return data

def frozen(root):
    paths=capture(['git','ls-files','-z'],root).split('\0')
    return {p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in paths if p and (root/p).is_file()}


def prepare(name, ref, archive=None):
    source = TEMP/name
    run(f'checkout-{name}', ['git','worktree','add','--detach',str(source),ref], seconds=120)
    if archive is not None:
        run('restore-retained-source-target',['tar','-I','zstd -T1','-xf',str(archive),
            '--no-same-owner','-C',str(TEMP)],seconds=180)
        assert (source/'.git').is_file(), 'fresh worktree .git must survive the mtime-preserving overlay'
        assert (TEMP/'target-seed').is_dir() and any((TEMP/'target-seed').rglob('*.rlib')), 'no compiled target restored; never fall back to empty target'
        capture(['git','diff','--exit-code','HEAD','--'],source)
    data = plan(source)
    save(EVIDENCE/f'{name}-plan.json',data)
    metadata = run(f'{name}-metadata',['cargo','metadata','--no-deps','--format-version','1',
                                     '--manifest-path','api/Cargo.toml'],cwd=source,seconds=120)
    parsed = json.loads(Path(metadata['stdout_log']).read_text())
    save(EVIDENCE/f'{name}-metadata.json',parsed)
    save(EVIDENCE/f'{name}-source-hashes.json', frozen(source))
    save(EVIDENCE/f'{name}-identity.json',{'sha':capture(['git','rev-parse','HEAD'],source),
         'status':capture(['git','status','--porcelain'],source),'plan':data})
    return source,data,parsed

def command_env(data, command, target):
    env = dict(os.environ, CARGO_TARGET_DIR=str(target),
               CARGO_BUILD_JOBS=str(data['runtime']['backend']['cargoJobs']),
               CARGO_INCREMENTAL='0',CARGO_NET_OFFLINE='true',
               CARGO_TERM_COLOR='never',TERM='dumb')
    for key,value in command.get('env',{}).items():
        if value is None:
            env.pop(key,None)
        else:
            env[key]=str(value)
    return env

def root_names(root,metadata):
    api=next(p for p in metadata['packages'] if p['name']=='api-server')
    aliases={}
    package=Path(api['manifest_path']).parent
    for target in api['targets']:
        source=Path(target['src_path'])
        identity=f'api-server/{",".join(target["kind"])}/{target["name"]}/{source.relative_to(root)}'
        # Cargo versions print package-relative or workspace-relative source headers.
        for base in [root/'api',package]:
            aliases[str(source.relative_to(base))]=identity
    return aliases

def cases(log, roots, mode):
    active=None
    rows=[]
    for line in Path(log).read_text(errors='replace').splitlines():
        header=re.search(r'\bRunning (?:unittests )?(.+?) \(',line)
        if header:
            active=roots.get(header.group(1))
            if active is None:
                raise RuntimeError(f'unknown Cargo root header: {line}')
        doc=re.search(r'\bDoc-tests (\S+)',line)
        if doc:
            active='api-server/doctest/'+doc.group(1)
        if mode=='list':
            match=re.match(r'(.+): (test|benchmark)$',line)
            if match:
                assert active, f'case has no target identity: {line}'
                rows.append([active,match.group(1),match.group(2)])
        else:
            match=re.match(r'test (.+?) \.\.\. (ok|FAILED|ignored)(?:[, ].*)?$',line)
            if match:
                assert active, f'execution has no target identity: {line}'
                rows.append([active,match.group(1),match.group(2)])
    return rows

def discover(name,root,data,metadata,target,executions):
    roots=root_names(root,metadata)
    out=[]
    for index,command in enumerate(data['tests']):
        env=command_env(data,command,target)
        argv=[command['command'],*command['args']]
        if '--' not in argv:
            argv+=['--']
        listing=run(f'{name}-list-{index}',[*argv,'--list'],cwd=root/command['cwd'],env=env,heavy=True)
        ignored=run(f'{name}-ignored-{index}',[*argv,'--list','--ignored'],
                    cwd=root/command['cwd'],env=env,heavy=True)
        listed=cases(listing['merged_log'],roots,'list')
        ignored_cases=Counter(tuple(x) for x in cases(ignored['merged_log'],roots,'list'))
        assert not (ignored_cases-Counter(tuple(x) for x in listed)), 'ignored listing outside original list'
        detailed=[]
        for case in listed:
            key=tuple(case)
            flag=ignored_cases[key]>0
            detailed.append([*case,flag])
            if flag:
                ignored_cases[key]-=1
        actual=cases(executions[index]['merged_log'],roots,'run')
        expected=Counter((x[0],x[1],'ignored' if x[3] else 'ok') for x in detailed)
        assert expected==Counter(tuple(x) for x in actual), f'{name} filter {index}: execution/list mismatch'
        assert any(x[2]=='ok' for x in actual), f'{name} filter {index}: no executed passing tests'
        out.append({'filter_index':index,'list':detailed,'actual':actual,
                    'original_argv':[command['command'],*command['args']]})
    save(EVIDENCE/f'{name}-target-qualified-cases.json',out)
    return out

def summarize_prior(path, output):
    count=0
    first=None
    last=None
    peaks={'memory_current':0,'tree_rss_sample':0,'max_process_vm_hwm':0,'anon':0,'file':0}
    targets=Counter()
    with Path(path).open('rb') as source, gzip.open(str(output)+'.samples.jsonl.gz','wb',compresslevel=1) as packed:
        for raw in source:
            packed.write(raw)
            row=json.loads(raw)
            count+=1
            first=first or row
            last=row
            for key in ['memory_current','tree_rss_sample','max_process_vm_hwm']:
                peaks[key]=max(peaks[key],row.get(key,0))
            for key in ['anon','file']:
                peaks[key]=max(peaks[key],row.get('memory_stat',{}).get(key,0))
            targets.update(p.get('crate_name','unknown') for p in row.get('processes',[]) if p.get('comm')=='rustc')
    stat_keys=['anon','file','pgscan','pgsteal','workingset_refault_file','workingset_refault_anon']
    small=lambda row: {key:row.get('memory_stat',{}).get(key) for key in stat_keys}
    save(Path(output),{'source_run':PRIOR_RUN,'sample_count':count,'sampled_peaks':peaks,
         'first':small(first or {}),'last':small(last or {}),'rustc_sample_presence':targets.most_common(8),
         'pressure_available_in_prior':bool(last and 'memory_pressure' in last),
         'not_runtime_PASS':True})
    print(json.dumps({'source_run':PRIOR_RUN,'sample_count':count,'sampled_peaks':peaks}),flush=True)

def cargo_fresh_summary():
    rows=[]
    for log in EVIDENCE.glob('*seed-normal-lib-prebuild.combined.log'):
        for line in log.open(errors='replace'):
            if not line.startswith('{'):
                continue
            try:
                row=json.loads(line.removeprefix('time: '))
            except json.JSONDecodeError:
                continue
            if row.get('reason')=='compiler-artifact':
                rows.append({'package_id':row['package_id'],'target':row['target']['name'],
                    'kind':row['target']['kind'],'fresh':row['fresh'],'profile':row['profile']})
    save(EVIDENCE/'restored-cargo-fresh.json',{'cache_start':'restored-partial-target; not cold',
        'fresh_artifacts':sum(r['fresh'] for r in rows),'rebuilt_artifacts':sum(not r['fresh'] for r in rows),
        'artifacts':rows,'limitation':'Interrupted rustc units have no compiler-artifact completion; no claim of complete warm cache'})
    print(json.dumps({'cache':'restored-partial','completed_fresh':sum(r['fresh'] for r in rows),
                      'completed_rebuilt':sum(not r['fresh'] for r in rows)}),flush=True)

def download_retained():
    prior=TEMP/'incoming'
    identity=run('verify-prior-artifact-id',['gh','api',
        'repos/taichuy/1flowbase/actions/artifacts/'+str(PRIOR_ARTIFACT_ID)],seconds=60)
    artifact=read(Path(identity['stdout_log']))
    assert artifact['id']==PRIOR_ARTIFACT_ID and artifact['name']==PRIOR_NAME and not artifact['expired']
    assert artifact['workflow_run']['id']==int(PRIOR_RUN)
    run('download-prior-artifact',['gh','run','download',PRIOR_RUN,'--name',PRIOR_NAME,
        '--repo','taichuy/1flowbase','--dir',str(prior)],seconds=600)
    archived=list(prior.rglob('seed-target-and-source.tar.zst'))
    assert len(archived)==1, 'exactly one fixed archive required'
    archive=archived[0]
    evidence=archive.parent
    retention=read(evidence/'seed-retention.json')
    assert retention['status']=='archived' and retention['source_sha']==SEED_SHA
    assert retention['sha256']==ARCHIVE_SHA and retention['bytes']==ARCHIVE_BYTES
    assert archive.stat().st_size==ARCHIVE_BYTES
    checksum=run('verify-retained-archive',['sha256sum',str(archive)],seconds=60)
    assert Path(checksum['stdout_log']).read_text().split()[0]==ARCHIVE_SHA
    assert retention['compiler']==capture(['rustc','-vV']), 'archive compiler differs; no cold fallback'
    assert retention['root_paths']==[str(TEMP/'seed-source'),str(TEMP/'target-seed')]
    members=run('validate-retained-members',['tar','-I','zstd -T1','-tf',str(archive)],seconds=90)
    for member in Path(members['stdout_log']).read_text().splitlines():
        parts=Path(member).parts
        assert parts and parts[0] in ('seed-source','target-seed') and '..' not in parts
        assert '.git' not in parts, 'archive must not overwrite the new worktree identity'
    save(EVIDENCE/'cache-resume.json',{'source_run':PRIOR_RUN,'artifact':PRIOR_NAME,
        'archive_sha256':ARCHIVE_SHA,'archive_bytes':ARCHIVE_BYTES,'archive_source_sha':SEED_SHA,
        'cache_state':'retained qualified W5 489 source/mtime/partial target; no cold fallback',
        'scope':'W5 ordinary API lib qualification; prior focused crate cases not rerun'})
    return archive,evidence

def retain_seed_cache(source, target, metadata):
    # One fixed archive, using the existing budget runner; no persistent cache service.
    global END
    END=max(END,time.monotonic()+360)
    archive=EVIDENCE/'seed-target-and-source.tar.zst'
    out=EVIDENCE/'seed-retention.json'
    if not target.exists():
        save(out,{'status':'no-target-created'})
        return
    metadata['actual_source_sha']=capture(['git','rev-parse','HEAD'],source)
    import resource
    cap=3_000_000_000
    command=[sys.executable,'-c',
        'import os,resource; resource.setrlimit(resource.RLIMIT_FSIZE,('+str(cap)+','+str(cap)+')); os.execvp("tar",'+repr(['tar','--exclude=seed-source/.git','-I','zstd -T1 -1','-cf',str(archive),'-C',str(TEMP),'seed-source','target-seed'])+')']
    try:
        row=run('retain-seed-target',command,seconds=300)
        digest=hashlib.sha256()
        with archive.open('rb') as stream:
            for chunk in iter(lambda:stream.read(1024*1024),b''):
                digest.update(chunk)
        save(out,{'status':'archived','archive':str(archive),'bytes':archive.stat().st_size,
                  'sha256':digest.hexdigest(),'compiler':capture(['rustc','-vV']),
                  'source_sha':metadata.get('actual_source_sha',metadata['source_sha']),'root_paths':[str(source),str(target)],
                  'cache_level':metadata,'note':'partial or complete as recorded; restore must revalidate Cargo freshness; source mtimes retained and .git excluded',
                  'archive_cost_seconds':row['wall_seconds']})
    except Exception as failure:
        archive.unlink(missing_ok=True)
        save(out,{'status':'archive-unavailable','reason':str(failure),'bounded_bytes':cap,'bounded_seconds':300})

ROOT_WRAPPER = "#!/usr/bin/env python3\nimport json,os,re,sys,time\nfrom pathlib import Path\nreal,args=sys.argv[1],sys.argv[2:]\ndef value(flag):\n    if flag in args:\n        return args[args.index(flag)+1]\n    return next((x.split('=',1)[1] for x in args if x.startswith(flag+'=')),None)\nmatch=(value('--crate-name')=='api_server' and value('--crate-type')=='lib'\n       and '--test' not in args and any(Path(x).name=='lib_root.rs' for x in args))\nmatch = match or (os.environ.get('ISSUE2818_RUSTC_MODE')=='observe-only' and value('--crate-name')=='console_core_registry_tests' and '--test' in args)\nenv=dict(os.environ)\neffective=list(args)\nif match:\n    mode=env.get('ISSUE2818_RUSTC_MODE','diagnostic')\n    if mode=='diagnostic':\n        effective+=['-Ztime-passes','-Ztime-passes-format=json']\n        if env.get('ISSUE2818_MONO_STATS')=='1':\n            effective+=['-Zdump-mono-stats','-Zdump-mono-stats-format=json']\n        env['RUSTC_BOOTSTRAP']='api_server'\n    auth=re.search(r'--jobserver-(?:auth|fds)=(\\d+),(\\d+)',env.get('CARGO_MAKEFLAGS',''))\n    fds={}\n    for fd in (map(int,auth.groups()) if auth else []):\n        try:\n            os.fstat(fd)\n            fds[str(fd)]='open'\n        except OSError:\n            fds[str(fd)]='closed'\n    row={'pid':os.getpid(),'monotonic':time.monotonic(),'mode':mode,'probe':'-Zhelp' in args,\n         'original_argv':[real,*args],'effective_argv':[real,*effective],\n         'bootstrap_for_this_exec':env.get('RUSTC_BOOTSTRAP'),'jobserver_fds':fds}\n    with open(env['ISSUE2818_ROOT_LOG'],'a') as log:\n        log.write(json.dumps(row)+'\\n')\n        log.flush()\n    if mode=='observe-only':\n        os._exit(86)\nos.execve(real,[real,*effective],env)\n"

def install_stage_wrapper(source,env):
    wrapper=TEMP/'seed-rustc-wrapper.py'
    wrapper.write_text(ROOT_WRAPPER)
    wrapper.chmod(0o700)
    (EVIDENCE/'seed-rustc-wrapper.py').write_text(ROOT_WRAPPER)
    assert not env.get('RUSTC_WRAPPER') and not env.get('RUSTC_WORKSPACE_WRAPPER')
    assert not env.get('RUSTC_BOOTSTRAP'), 'bootstrap must be scoped only to matched rustc exec'
    diagnostic={**env,'CARGO_BUILD_JOBS':'1','RUSTC_WRAPPER':str(wrapper),
        'ISSUE2818_RUSTC_MODE':'diagnostic',
        'ISSUE2818_ROOT_LOG':str(EVIDENCE/'root-rustc-invocations.jsonl')}
    real=capture(['rustup','which','rustc'])
    help_row=run('rustc-stage-help',[str(wrapper),real,'--crate-name','api_server',
        '--crate-type','lib',str(source/'api/apps/api-server/src/lib_root.rs'),'-Zhelp'],
        cwd=source/'api',env=diagnostic,seconds=30)
    help_text=Path(help_row['stdout_log']).read_text()
    assert 'time-passes' in help_text and 'time-passes-format' in help_text
    stats={'status':'skipped','reason':'1.99.1 help does not establish both mono-stat flags/format'}
    format_help=next((line for line in help_text.splitlines() if 'dump-mono-stats-format' in line),'')
    if 'dump-mono-stats' in help_text and 'json' in format_help:
        probe=run('mono-stats-help',[str(wrapper),real,'--crate-name','api_server','--crate-type','lib',
            str(source/'api/apps/api-server/src/lib_root.rs'),'-Zhelp'],
            cwd=source/'api',env={**diagnostic,'ISSUE2818_MONO_STATS':'1'},seconds=30,expected='optional')
        if probe['exit_code']==0:
            diagnostic['ISSUE2818_MONO_STATS']='1'
            stats={'status':'enabled-root-only','flags':['-Zdump-mono-stats','-Zdump-mono-stats-format=json'],
                   'not_RSS_percentage_allocation':True}
        else:
            stats={'status':'skipped','reason':'optional format/flag probe rejected','exit_code':probe['exit_code']}
    save(EVIDENCE/'mono-stats-probe.json',stats)
    return diagnostic

def stage_summary():
    path=EVIDENCE/'root-rustc-invocations.jsonl'
    invocations=[json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
    completed=[]
    for log in EVIDENCE.glob('*seed-normal-lib-prebuild.combined.log'):
        for line in log.open(errors='replace'):
            try:
                row=json.loads(line.removeprefix('time: '))
            except json.JSONDecodeError:
                continue
            if isinstance(row,dict) and ('pass' in row or 'pass_name' in row):
                completed.append(row)
    builds=[r for r in invocations if r['mode']=='diagnostic' and not r['probe']]
    blocked=[r for r in invocations if r['mode']=='observe-only']
    data={'root_match_count':len(invocations),'diagnostic_root_execs':len(builds),
        'default_root_recompile_blocked':len(blocked),'invocations':invocations,
        'last_completed_pass':completed[-1] if completed else None,'completed_pass_count':len(completed),
        'key_completed_passes':[r for r in completed if r.get('pass') in ['monomorphization_collector_graph_walk','partition_and_assert_distinct_symbols','generate_crate_metadata','find_cgu_reuse','LLVM_passes','codegen_to_LLVM_IR']],
        'limits':'time-passes RSS values are completed-pass endpoints, not peaks; interrupted pass may have no trailing row; observations do not prove the exact OOM phase or pure jobs1 speed effect'}
    save(EVIDENCE/'compiler-stage-summary.json',data)
    print(json.dumps({'diagnostic_root_execs':len(builds),'default_recompile_blocked':len(blocked),
        'last_completed_pass':data['last_completed_pass']}),flush=True)
    return data

def promote_w2(source,candidate,prior_evidence):
    changed=capture(['git','diff','--name-only',SEED_SHA,candidate],source).splitlines()
    assert len(changed)==19 and set(changed)==set(W2_ALLOWED), 'candidate must change exactly the approved existing 19 files'
    run('checkout-w2-candidate',['git','checkout','--detach',candidate],cwd=source,seconds=60)
    assert capture(['git','rev-parse','HEAD'],source)==candidate
    capture(['git','diff','--exit-code','HEAD','--'],source)
    old=read(prior_evidence/'seed-source-source-hashes.json')
    new=frozen(source)
    assert old.keys()==new.keys()
    assert all(new[p]==h for p,h in old.items() if p not in W2_ALLOWED), 'protected source/config/fixture changed'
    for item in W2_PROTECTED:
        assert hashlib.sha256((source/item['path']).read_bytes()).hexdigest()==item['sha256'], item['path']
    for file in W2_LEAVES:
        current=(source/file).read_text()
        stripped=re.sub(r'(?m)^[ \t]*#\[test\]\n','',current)
        original=subprocess.check_output(['git','show',SEED_SHA+':'+file],cwd=source).decode()
        assert stripped==original, 'whitebox body/helper/signature differs: '+file
    save(EVIDENCE/'w2-source-validation.json',{'source_sha':candidate,'archive_source_sha':SEED_SHA,
        'allowed_and_actual_changes':changed,'protected_hashes_match':True,'leaf_bodies_unchanged_except_test_attrs':True,
        'candidate_checkout_after_archive_restore':True})
    save(EVIDENCE/'w2-expected-map.json',{'source_map_sha256':W2_MAP_SHA,'identity_mapping':W2_MAP,
        'native_seven':W2_NATIVE_SEVEN,'old_runtime_evidence':'not available; no old PASS invented'})

OWNER_BUILDER = r"""
const path=require('node:path'),root=process.argv[1],owner=process.argv[2],filter=process.argv[3];
const runtime=JSON.parse(process.env.ISSUE2818_FROZEN_RUNTIME);
const commands=require(path.join(root,'scripts/node/verify/backend.js')).buildBackendTestCommands({
  repoRoot:root,env:process.env,cargoJobs:1,cargoTestThreads:runtime.backend.cargoTestThreads,
  incremental:runtime.backend.incremental,selection:{packages:['api-server'],
  targetArgs:['--test',owner,'--features','test-support'],filter}});
process.stdout.write(JSON.stringify(commands.find(c=>c.command==='cargo')));
"""

def execute_w2_owner(source,data,cargo_meta,target,diagnostic,owner,filter,names,label):
    command=json.loads(capture(['node','-e',OWNER_BUILDER,str(source),owner,filter],source))
    env={**command_env(data,command,target),'RUSTC_WRAPPER':diagnostic['RUSTC_WRAPPER'],
        'ISSUE2818_RUSTC_MODE':'observe-only','ISSUE2818_ROOT_LOG':diagnostic['ISSUE2818_ROOT_LOG']}
    argv=[command['command'],*command['args']]
    roots=root_names(source,cargo_meta)
    identities={v for v in roots.values() if '/test/'+owner+'/' in v}
    assert len(identities)==1
    identity=next(iter(identities))
    listing=run(label+'-list',[*argv,'--list'],cwd=source/command['cwd'],env=env,heavy=True,seconds=600)
    ignored=run(label+'-ignored',[*argv,'--list','--ignored'],cwd=source/command['cwd'],env=env,heavy=True,seconds=600)
    listed=cases(listing['merged_log'],roots,'list')
    assert Counter(map(tuple,listed))==Counter((identity,name,'test') for name in names), 'compiled owner/name list differs: '+label
    assert not cases(ignored['merged_log'],roots,'list'), 'unexpected ignored case: '+label
    execution=run(label+'-execute',argv,cwd=source/command['cwd'],env=env,heavy=True,seconds=600)
    actual=cases(execution['merged_log'],roots,'run')
    assert Counter(map(tuple,actual))==Counter((identity,name,'ok') for name in names)
    save(EVIDENCE/(label+'-cases.json'),{'target':owner,'argv':argv,'listed':listed,'ignored':[],
        'actual':actual,'count':len(actual),'executed_once':True})
    return listed,actual

def w2_cost_summary():
    groups={'ordinary_lib':[],'private_owner_new_compile_and_24':[],'native_remaining_7':[],
            'seed_queries_and_new_contracts_compile':[],'listing_and_other_preparation':[]}
    for row in RESULTS:
        label=row['label']
        group=('ordinary_lib' if label=='seed-normal-lib-prebuild' else
               'private_owner_new_compile_and_24' if label.startswith('w2-private') else
               'native_remaining_7' if label.startswith('w2-native') else
               'seed_queries_and_new_contracts_compile' if label.startswith('seed-query-') and 'unit-graph' not in label else
               'listing_and_other_preparation')
        groups[group].append({'label':label,'wall_seconds':row['wall_seconds'],
                              'classification':row['classification'],'receipt':row['receipt']})
    save(EVIDENCE/'w2-cost-summary.json',{'source_sha':os.environ.get('CANDIDATE_SHA'),
        'serial_controlled_feasibility_not_PR_job_topology':True,'cost_groups':groups,
        'total_measured_command_seconds':sum(r['wall_seconds'] for r in RESULTS),
        'limits':'includes failures/new private/native/contracts work; no historical thin/private A/B or double-compilation removal claim; upload/setup costs remain workflow-level'})

def collect_mono_files(source,target):
    files=[]
    for directory in [source/'api',target/'debug/deps']:
        if not directory.exists():
            continue
        for path in directory.glob('*mono*'):
            if not path.is_file() or path.suffix not in ('.json','.csv'):
                continue
            row={'path':str(path),'bytes':path.stat().st_size}
            if path.stat().st_size<=64*1024*1024:
                packed=EVIDENCE/(path.name+'.gz')
                with path.open('rb') as incoming,gzip.open(packed,'wb',compresslevel=1) as outgoing:
                    for chunk in iter(lambda:incoming.read(1024*1024),b''):
                        outgoing.write(chunk)
                row['gzip']=str(packed)
            else:
                row['retained_in_native_tar_only']=True
            files.append(row)
    save(EVIDENCE/'mono-stat-files.json',{'files':files,'meaning':'definition/instantiation attribution only; not RSS allocation percentages'})

OLD_API_MONO_SHA256 = '3d98449c62ba823e2f2dd3c5261639f0d53719a39258c0ae8bdee8957ecb47ba'
API_SOURCE_SHA256 = {'api/crates/interface-runtime/src/invocation/kernel.rs': 'f9062a67513dce43829d900480468b18365e44bbba74609b08bd934e6cf9934f', 'api/crates/interface-runtime/src/invocation.rs': '8815cbd2b47fa721ef0e4992fa77025398be839be2d5db30688bb38b6e7bbd37', 'api/crates/interface-runtime/src/invocation/unary.rs': '021739e1fe12a67ae21b8b7592581e1e368f714d68d489e0d41cf94acf56cab1', 'api/crates/interface-runtime/src/invocation/unary/bridge.rs': '496deb4a33b4853b726b91980a5cca4dbc9436eb8f3848a4f0d00dfddaac492b', 'api/crates/interface-runtime/src/invocation/policy.rs': None, 'api/crates/interface-runtime/src/_tests/invocation_tests.rs': 'b4eeacd4faf42196926214a5cd81a3d647d2cd495acb87cc8e645f47c53f8d26', 'api/crates/interface-runtime/src/_tests/review_remediation_tests/decision_order.rs': '3499278ab92a714ad790fa9c42f14b2e9b086f8be265f5556cb8c7b482b3cf96'}


WITHDRAWAL_ALLOWED = ['.agents/skills/qa-evaluation/references/backend/console-settings-registration-gate.md', '.github/workflows/event-lifecycle.yml', '.github/workflows/notification-subscription.yml', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/data_model_and_docs.rs', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/infrastructure_and_mcp.rs', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/plugins_and_models.rs', 'api/apps/api-server/src/routes/_tests/file_console_route_assembly_tests.rs', 'api/apps/api-server/src/routes/_tests/mod.rs', 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'api/apps/api-server/src/routes/console_route_assembly.rs', 'api/apps/api-server/src/routes/console_route_assembly/_tests/assembly.rs', 'api/apps/api-server/src/routes/mod.rs', 'api/apps/api-server/src/test_support/mod.rs', 'api/apps/api-server/tests/README.md', 'api/apps/api-server/tests/console_core_registry_tests.rs', 'scripts/node/agent-logs-exchange/ci-api.cjs', 'scripts/node/console-operation-registry-hygiene/README.md', 'scripts/node/console-operation-registry-hygiene/_tests/core.test.js', 'scripts/node/console-operation-registry-hygiene/compiled-assembly-checks.js']
WITHDRAWAL_BEFORE_HASHES = {'.agents/skills/qa-evaluation/references/backend/console-settings-registration-gate.md': 'dbb2ee4d4102f6dff92628a71eaebe387a2b42211e6758f5dec772ca9c836e29', '.github/workflows/notification-subscription.yml': 'b8b4e33e9e3665566404e955f5f4f9c1a5180bcc8aef492862ef060d326aacb9', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs': '6163a6f9ec09f1da5f8cdf64d97a87d70ddacefcc4271fb073e663fc0a5662d9', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/data_model_and_docs.rs': 'd9c3028b4b8f9d505e3c2cf346e366d80ac5569b156d2af75a927c880ae103e5', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/infrastructure_and_mcp.rs': 'be553496bc98b30968f213b7258a67720c903af0fa819cb8cf16af54aeba14bd', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/plugins_and_models.rs': '1b6cc2e3d0341b2ef29c4e98ba4ef1035a94aa27ef8d03c8381ac3ae0e9e40a8', 'api/apps/api-server/src/routes/_tests/file_console_route_assembly_tests.rs': 'd37f6525b39d776b09af1078283b65a52c5a27c82d9b077da8d41ef9316b511a', 'api/apps/api-server/src/routes/_tests/mod.rs': '7989b5d5348b63b2a31a56a80d8b8e624ff6c215517492295201019fe6b71601', 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs': '367bf77d9f4c720c4c96d3a97296278a4f87ad7981966f002d56f2cfd51e5722', 'api/apps/api-server/src/routes/console_route_assembly.rs': '1e0fbf2ef2087f10683fcf593a41062506ffff528be216a77aedfae9d8af2126', 'api/apps/api-server/src/routes/console_route_assembly/_tests/assembly.rs': 'b662ca63e7ae84902feff696db80467dc95183782a6a7d03cb72343a51fed525', 'api/apps/api-server/src/routes/mod.rs': 'bb11f86c2cef95fd325e6e1d8df250471d917e46d860407ab04b0b8c5df729c3', 'api/apps/api-server/src/test_support/mod.rs': 'fa44348eb5c423f80476bccdae4e95ed203974f9693a18ac91f8b8a203c6c38e', 'api/apps/api-server/tests/README.md': '754cd4ce1686e6d77ce7bdf4fdda03f6654542e91319c026389994500a5e66af', 'api/apps/api-server/tests/console_core_registry_tests.rs': '1ce1feb2208103a79d3817675eb12e44c1623bab23725744df4a7fe10e9bb87d', 'scripts/node/agent-logs-exchange/ci-api.cjs': '0d620baf12e1a1bf885b6be338827db625387abeff0b8d59a3e1773b99b80d1d', 'scripts/node/console-operation-registry-hygiene/README.md': '5371a73f5bd4a8265248e52d2babfa649d6d19450e3b99d959e2758c3b0caf23', 'scripts/node/console-operation-registry-hygiene/_tests/core.test.js': '5d069e60ecdaa3248a3cb13255d674a5463b1b3226282e600c0d5e0dfac4094f', 'scripts/node/console-operation-registry-hygiene/compiled-assembly-checks.js': '96dad3a01084fbbf0c7549b09de9fdd3a49ae4bbb4807db882c1f936a36d14c7', '.github/workflows/event-lifecycle.yml': 'fb0a4cc4be8444e511fc7e1fa616ade1e4e1670a2b5dbab51083423994fa8048'}
WITHDRAWAL_AFTER_HASHES = {'.agents/skills/qa-evaluation/references/backend/console-settings-registration-gate.md': '275b3637abadd1833821b5443194c583686ab0ce20b52bf1938104148c1d253b', '.github/workflows/notification-subscription.yml': 'c009a86984fea6d7e5073b3bfdbc9f63c111f75cf1d0eca3a24419751e9fa0d5', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs': 'c53a3a11065b3f3e9d7bf45e070d24da4e5f5bf42159894b59f8068378a967ad', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/data_model_and_docs.rs': 'b0bacdc497b2e53e195467b6f6e08e2b2ae609c8c9ab0280a87b6d82e5e6eb09', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/infrastructure_and_mcp.rs': '306688f6a1313ded6845c4d96dc4385b92918074c51b0b2c38c66b5ddda50e26', 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/plugins_and_models.rs': '1185a477b7446a8b1c4a4fb082c2f677c3d46121308ed702a024562ffc028f71', 'api/apps/api-server/src/routes/_tests/file_console_route_assembly_tests.rs': '4071b0b1c1b030e1ed204a89ecbfc3751d48d232cd3ba23b3392df2dd2b1d373', 'api/apps/api-server/src/routes/_tests/mod.rs': 'fe693eee06273dcdcda873178b8d38a1e9a007b770ad9534b5ce7a561afc8cc8', 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs': 'ceaffab1cddb69ed1506471beb96a9cd4921795b6edea77c5732792daf89288c', 'api/apps/api-server/src/routes/console_route_assembly.rs': '929aa79fde6a8a23529d8eb98cc6b60617da9c0f204295127cd5b47746174e5e', 'api/apps/api-server/src/routes/console_route_assembly/_tests/assembly.rs': '70d1d7a5c6a140c317e291f79ecab442ad6bc2960d454a9bc0e9492b0885614a', 'api/apps/api-server/src/routes/mod.rs': 'e2e97f2acbfadf0c23fc528139142fabc0daefec91333cf91dfca3142b823e90', 'api/apps/api-server/src/test_support/mod.rs': 'ed07072559c153bd6dc6f9d91f9ea86f54ca9a8298ffe44efaa647cdbbe2ec9e', 'api/apps/api-server/tests/README.md': '24d84cbb7b08085e266f9de20920ae39a0605d471d7f71cae71d2d0a50f6e57e', 'api/apps/api-server/tests/console_core_registry_tests.rs': '65506395803782702d76adf3582fb0875121fb2120962c93414234bea1edfea0', 'scripts/node/agent-logs-exchange/ci-api.cjs': 'ed0e940a94e2d8c2b25e4147a538d348661a5c20bd962c2e03a973a235512506', 'scripts/node/console-operation-registry-hygiene/README.md': '26b6d2ad0e5467fd279ad60cc3f11341d505a92e99eafa2090ecb304f5c7307c', 'scripts/node/console-operation-registry-hygiene/_tests/core.test.js': '2b9f0d6b8b52b1ba4998a5c798bab4d74d4f7f226c413f50084d2f2d99ffed59', 'scripts/node/console-operation-registry-hygiene/compiled-assembly-checks.js': '777e1a57babfbddcc73dcdec80ef13c56ceb12df1806cb32836ca11740b77b0a', '.github/workflows/event-lifecycle.yml': '72af1d77d337f8fbf7d8be3d6cbb13b2f60118bbce69b8193505f4900d0ddaa1'}
NATIVE31_NAMES = ['ac_002_console_health_is_compiled_and_not_exposed_by_the_base_router', 'ac_003_unknown_core_operation_metadata_fails_instead_of_becoming_other', 'ac_003_every_console_route_is_owned_by_a_compiled_authorization_operation', 'ac_003_005_all_console_routes_have_distinct_static_english_interface_metadata', 'ac_009_core_compiled_catalog_resolves_every_active_display_reference_in_both_locales', 'ac_002_009_operation_semantics_are_explicit_before_legacy_mapping', 'ac_013_role_console_policy_workers_have_explicit_interface_metadata', 'routes::console_route_assembly::tests::migrated_assembly_contains_every_console_router_owner_assembly', 'routes::console_route_assembly::tests::complete_operation_specifications_match_the_production_route_set', 'routes::console_route_assembly::tests::operation_specification_exact_set_reports_every_difference', 'routes::_tests::console_route_assembly_tests::console_route_assembly_unclassified_route_fails_coverage', 'routes::_tests::console_route_assembly_tests::console_route_assembly_duplicate_ownership_fails_coverage', 'routes::_tests::console_route_assembly_tests::migrated_real_core_console_route_assembly_has_compiled_coverage', 'routes::_tests::console_route_assembly_tests::applications_routes_compile_exact_operations_and_resource_metadata', 'routes::_tests::console_route_assembly_tests::applications_closed_set_rejects_duplicate_or_missing_binding', 'routes::_tests::console_route_assembly_tests::application_api_orchestration_runtime_routes_compile_exact_operations', 'routes::_tests::console_route_assembly_tests::data_model_and_docs::data_model_docs_and_data_source_routes_compile_exact_operations', 'routes::_tests::console_route_assembly_tests::infrastructure_and_mcp::infrastructure_mcp_and_user_api_key_routes_compile_exact_operations', 'routes::_tests::console_route_assembly_tests::plugins_and_models::ac_002_ac_013_plugins_and_models_owner_routes_have_explicit_assembly_ownership', 'routes::_tests::file_console_route_assembly_tests::file_owner_routes_compile_from_exact_assemblies', 'routes::_tests::file_console_route_assembly_tests::file_owner_routes_preserve_authenticated_data_acl_and_settings_operations', 'routes::_tests::file_console_route_assembly_tests::temporary_file_contracts_compile_without_projecting_credentials', 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_members_route_bindings_are_explicit_and_stable', 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_permissions_route_binding_is_explicit_and_stable', 'routes::_tests::settings_owner_console_route_assembly_tests::ac_001_ui_management_route_bindings_are_explicit_and_granular', 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_roles_route_bindings_are_explicit_and_stable', 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_workspace_route_bindings_are_explicit_and_stable', 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_workspaces_route_binding_is_explicit_and_stable', 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_auth_center_route_bindings_are_explicit_and_stable', 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_system_route_bindings_are_explicit_and_stable', 'routes::_tests::settings_owner_console_route_assembly_tests::organization_console_route_assembly_keeps_each_operation_independent']
WITHDRAWAL_IDENTITY_MAP = [{'assertion_source': 'api/apps/api-server/src/routes/console_route_assembly/_tests/assembly.rs', 'function': 'migrated_assembly_contains_every_console_router_owner_assembly', 'full_test_name': 'routes::console_route_assembly::tests::migrated_assembly_contains_every_console_router_owner_assembly', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/console_route_assembly/_tests/assembly.rs', 'function': 'complete_operation_specifications_match_the_production_route_set', 'full_test_name': 'routes::console_route_assembly::tests::complete_operation_specifications_match_the_production_route_set', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/console_route_assembly/_tests/assembly.rs', 'function': 'operation_specification_exact_set_reports_every_difference', 'full_test_name': 'routes::console_route_assembly::tests::operation_specification_exact_set_reports_every_difference', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'function': 'console_route_assembly_unclassified_route_fails_coverage', 'full_test_name': 'routes::_tests::console_route_assembly_tests::console_route_assembly_unclassified_route_fails_coverage', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'function': 'console_route_assembly_duplicate_ownership_fails_coverage', 'full_test_name': 'routes::_tests::console_route_assembly_tests::console_route_assembly_duplicate_ownership_fails_coverage', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'function': 'migrated_real_core_console_route_assembly_has_compiled_coverage', 'full_test_name': 'routes::_tests::console_route_assembly_tests::migrated_real_core_console_route_assembly_has_compiled_coverage', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'function': 'applications_routes_compile_exact_operations_and_resource_metadata', 'full_test_name': 'routes::_tests::console_route_assembly_tests::applications_routes_compile_exact_operations_and_resource_metadata', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'function': 'applications_closed_set_rejects_duplicate_or_missing_binding', 'full_test_name': 'routes::_tests::console_route_assembly_tests::applications_closed_set_rejects_duplicate_or_missing_binding', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests.rs', 'function': 'application_api_orchestration_runtime_routes_compile_exact_operations', 'full_test_name': 'routes::_tests::console_route_assembly_tests::application_api_orchestration_runtime_routes_compile_exact_operations', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/file_console_route_assembly_tests.rs', 'function': 'file_owner_routes_compile_from_exact_assemblies', 'full_test_name': 'routes::_tests::file_console_route_assembly_tests::file_owner_routes_compile_from_exact_assemblies', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/file_console_route_assembly_tests.rs', 'function': 'file_owner_routes_preserve_authenticated_data_acl_and_settings_operations', 'full_test_name': 'routes::_tests::file_console_route_assembly_tests::file_owner_routes_preserve_authenticated_data_acl_and_settings_operations', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/file_console_route_assembly_tests.rs', 'function': 'temporary_file_contracts_compile_without_projecting_credentials', 'full_test_name': 'routes::_tests::file_console_route_assembly_tests::temporary_file_contracts_compile_without_projecting_credentials', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_002_013_members_route_bindings_are_explicit_and_stable', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_members_route_bindings_are_explicit_and_stable', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_002_013_permissions_route_binding_is_explicit_and_stable', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_permissions_route_binding_is_explicit_and_stable', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_001_ui_management_route_bindings_are_explicit_and_granular', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_001_ui_management_route_bindings_are_explicit_and_granular', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_002_013_roles_route_bindings_are_explicit_and_stable', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_roles_route_bindings_are_explicit_and_stable', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_002_013_workspace_route_bindings_are_explicit_and_stable', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_workspace_route_bindings_are_explicit_and_stable', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_002_013_workspaces_route_binding_is_explicit_and_stable', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_workspaces_route_binding_is_explicit_and_stable', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_002_013_auth_center_route_bindings_are_explicit_and_stable', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_auth_center_route_bindings_are_explicit_and_stable', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'ac_002_013_system_route_bindings_are_explicit_and_stable', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::ac_002_013_system_route_bindings_are_explicit_and_stable', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/settings_owner_console_route_assembly_tests.rs', 'function': 'organization_console_route_assembly_keeps_each_operation_independent', 'full_test_name': 'routes::_tests::settings_owner_console_route_assembly_tests::organization_console_route_assembly_keeps_each_operation_independent', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/data_model_and_docs.rs', 'function': 'data_model_docs_and_data_source_routes_compile_exact_operations', 'full_test_name': 'routes::_tests::console_route_assembly_tests::data_model_and_docs::data_model_docs_and_data_source_routes_compile_exact_operations', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/infrastructure_and_mcp.rs', 'function': 'infrastructure_mcp_and_user_api_key_routes_compile_exact_operations', 'full_test_name': 'routes::_tests::console_route_assembly_tests::infrastructure_and_mcp::infrastructure_mcp_and_user_api_key_routes_compile_exact_operations', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}, {'assertion_source': 'api/apps/api-server/src/routes/_tests/console_route_assembly_tests/plugins_and_models.rs', 'function': 'ac_002_ac_013_plugins_and_models_owner_routes_have_explicit_assembly_ownership', 'full_test_name': 'routes::_tests::console_route_assembly_tests::plugins_and_models::ac_002_ac_013_plugins_and_models_owner_routes_have_explicit_assembly_ownership', 'current_target': 'api_private_routes', 'restored_target': 'console_core_registry_tests', 'ignored': False}]
RESULTS_PREFIX = 'issue2818-clean-checks-results-'

REUSED_NATIVE_ARTIFACT = {'executable': '/home/runner/work/_temp/issue2818/target-seed/debug/deps/console_core_registry_tests-e7a7c958679d3196', 'fresh': False, 'package_id': 'path+file:///home/runner/work/_temp/issue2818/seed-source/api/apps/api-server#0.6.3', 'profile': {'debug_assertions': True, 'debuginfo': 0, 'opt_level': '0', 'overflow_checks': True, 'test': True}, 'target': {'crate_types': ['bin'], 'doc': False, 'doctest': False, 'edition': '2021', 'kind': ['test'], 'name': 'console_core_registry_tests', 'src_path': '/home/runner/work/_temp/issue2818/seed-source/api/apps/api-server/tests/console_core_registry_tests.rs', 'test': True}}

CLEAN_CHECK_COMMANDS = [['cargo', 'check', '--package', 'control-plane', '--package', 'api-server', '--package', 'control-plane-postgres-tests', '--jobs', '1'], ['cargo', 'check', '--package', 'api-server', '--all-targets', '--jobs', '1']]

CRATE_CANDIDATE = '7bb249e47d29dce87a291677884bd42962aa2046'
CRATE_ALLOWED = ['api/crates/interface-runtime/src/_tests/review_remediation_tests/decision_order.rs', 'api/crates/interface-runtime/src/decision.rs', 'api/crates/interface-runtime/src/hook.rs', 'api/crates/interface-runtime/src/invocation.rs', 'api/crates/interface-runtime/src/invocation/kernel.rs', 'api/crates/interface-runtime/src/invocation/policy.rs']

def crate_artifacts(label=None):
    rows=[]
    for log in EVIDENCE.glob('*'+(label or 'interface-runtime-lib')+'.combined.log'):
        for line in log.open(errors='replace'):
            try:
                row=json.loads(line)
            except json.JSONDecodeError:
                continue
            if row.get('reason')=='compiler-artifact':
                rows.append({'package_id':row['package_id'],'target':row['target'],
                    'profile':row['profile'],'fresh':row['fresh'],'executable':row.get('executable')})
    save(EVIDENCE/((label+'-artifacts.json') if label else 'interface-runtime-artifacts.json'),{'artifacts':rows,
        'fresh':sum(r['fresh'] for r in rows),'rebuilt':sum(not r['fresh'] for r in rows),
        'note':'standalone crate test feature/profile graph; do not assume prior API 509 count'})
    return rows

CRATE_TEST_PATHS = ['api/crates/interface-runtime/src/_tests/finalization_driver_tests.rs']
BASELINE_MOUNT_FILE = 'api/crates/interface-runtime/src/finalization.rs'
BASELINE_CHILD_MOUNT = '\n#[cfg(test)]\n#[path = "_tests/finalization_driver_tests.rs"]\nmod driver_tests;\n'
BASELINE_MOUNT_OFFSET = 3615
BASELINE_ORIGINAL_SHA256 = '933fcb4f7648f14aaa8fde646f9d09792340423a387b3883a7d4cf6d8654b656'
CRATE_FILE_SHA256 = {'api/crates/interface-runtime/src/finalization.rs': 'cf892d97b4fac76a510e9c62e6dd9fc63617f5f8e40e281d62325555be910ca4', 'api/crates/interface-runtime/src/_tests/finalization_driver_tests.rs': 'fc5469b9e28449070148d2cdd27269b78209b3c8d003aa5de52a693d8db5a747'}

def qualify_interface_phase(side,source,target,env,argv,roots,identity,native):
    label=side+'-interface-runtime-lib'
    executed=run(label,argv,cwd=source/'api',env=env,heavy=True,seconds=600)
    listed=run(side+'-interface-runtime-list',[*argv,'--list'],cwd=source/'api',env=env,heavy=True,seconds=120)
    ignored=run(side+'-interface-runtime-ignored',[*argv,'--list','--ignored'],cwd=source/'api',env=env,heavy=True,seconds=120)
    listing=cases(listed['merged_log'],roots,'list')
    ignored_rows=cases(ignored['merged_log'],roots,'list')
    ignored_cases=Counter(map(tuple,ignored_rows))
    expected=Counter()
    detailed=[]
    for root,name,kind in listing:
        key=(root,name,kind)
        flag=ignored_cases[key]>0
        detailed.append([root,name,kind,flag])
        expected[(root,name,'ignored' if flag else 'ok')]+=1
        if flag:ignored_cases[key]-=1
    assert not +ignored_cases
    actual=cases(executed['merged_log'],roots,'run')
    assert expected==Counter(map(tuple,actual)) and any(row[2]=='ok' for row in actual), side+': incomplete/zero/failed actual crate tests'
    artifacts=crate_artifacts(label)
    assert any(r['target']['name']==native['name'] and r['profile']['test'] and r['executable'] for r in artifacts)
    result={'side':side,'identity':identity,'argv':argv,'list_with_ignored':detailed,'actual':actual,
        'passed':sum(r[2]=='ok' for r in actual),'failed':sum(r[2]=='FAILED' for r in actual)}
    save(EVIDENCE/(side+'-crate-test-cases.json'),result)
    return result

FOCUSED_TEST_FILE = 'api/crates/interface-runtime/src/_tests/review_remediation_tests/decision_order.rs'
CRATE_TEST_SHA256 = '415594bd8ad4c7b546a0bdbbdf06748864116cb3b1c1b09b8084ec5e3522ee2f'
FOCUSED_FILTERS = ['_tests::review_remediation_tests::decision_order::','_tests::invocation_tests::','_tests::review_remediation_tests::finalization_tests::']

def follow_withdrawal(run_id,out_dir,control_sha):
    from concurrent.futures import ThreadPoolExecutor
    from datetime import datetime,timezone
    import urllib.request,urllib.error,zipfile
    out=Path(out_dir).resolve();out.mkdir(parents=True,exist_ok=True)
    def now():return datetime.now(timezone.utc).isoformat()
    start=time.monotonic()
    with (out/'watch.log').open('wb') as log:
        watch=subprocess.run(['gh','run','watch',str(run_id),'--interval','30','--exit-status'],stdout=log,stderr=subprocess.STDOUT,timeout=2400)
    watched={'exit':watch.returncode,'elapsed_seconds':time.monotonic()-start,'finished_at':now(),'log':str(out/'watch.log')}
    record=json.loads(subprocess.check_output(['gh','run','view',str(run_id),'--json','databaseId,status,conclusion,headSha,url,attempt,jobs'],text=True,timeout=45))
    assert record['status']=='completed' and record['headSha']==control_sha
    save(out/'run-final.json',record)
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self,req,fp,code,msg,headers,newurl):return None
    def signed_url(url):
        token=subprocess.check_output(['gh','auth','token'],text=True).strip()
        opener=urllib.request.build_opener(NoRedirect)
        try:
            opener.open(urllib.request.Request(url,headers={'Authorization':'Bearer '+token}),timeout=25)
            raise RuntimeError('expected GitHub signed redirect')
        except urllib.error.HTTPError as error:
            assert error.code==302,error.code
            return error.headers['Location']
        finally:token=None
    def stdout_cause():
        began=time.monotonic();result={'run_id':run_id,'source_sha':CRATE_CANDIDATE,'control_sha':control_sha,'byte_cap':262144,'scope':'bounded current job stdout, selected phase JSON/error text only','started_at':now()}
        try:
            url=signed_url('https://api.github.com/repos/taichuy/1flowbase/actions/jobs/'+str(record['jobs'][0]['databaseId'])+'/logs')
            with urllib.request.urlopen(url,timeout=30) as response:raw=response.read(result['byte_cap']+1)
            result.update(bytes_read=min(len(raw),result['byte_cap']),truncated=len(raw)>result['byte_cap'])
            phases=[];errors=[]
            for line in raw[:result['byte_cap']].decode(errors='replace').splitlines():
                if 'ci_phase_summary' in line:
                    try:row=json.loads(line[line.index('{'):])['ci_phase_summary']
                    except (ValueError,KeyError):continue
                    assert row['source_sha'] in [SEED_SHA,CRATE_CANDIDATE];phases.append(row)
                elif any(word in line for word in ['RuntimeError','AssertionError','error:','panicked']):errors.append(line[:2000])
            result.update(phases=phases,first_failure=next((r for r in phases if r['exit']!=0 or r['classification']!='exit'),None),errors=errors[-8:])
        except Exception as error:result.update(transport_error=type(error).__name__+': '+str(error))
        result.update(elapsed_seconds=time.monotonic()-began,ready_at=now())
        save(out/'stdout-phase-attribution.json',result)
        print(json.dumps({'stdout_first_attribution':result}),flush=True)
        return result
    def small_results():
        began=time.monotonic();result={'run_id':run_id,'started_at':now(),'transport':'ordinary whole small ZIP via7897/TLS; no Range/cache','exit':1}
        try:
            assert os.environ.get('HTTPS_PROXY')=='http://127.0.0.1:7897'
            meta=json.loads(subprocess.check_output(['gh','api','repos/taichuy/1flowbase/actions/runs/'+str(run_id)+'/artifacts'],text=True,timeout=45));save(out/'artifact-meta.json',meta)
            name=RESULTS_PREFIX+str(run_id)+'-'+str(record['attempt'])
            light=next(a for a in meta['artifacts'] if a['name']==name)
            assert not light['expired'] and light['size_in_bytes']<=16_000_000 and light['workflow_run']['head_sha']==control_sha
            result['artifact']=light;url=signed_url(light['archive_download_url']);archive=out/'small-results.zip';received=0
            with urllib.request.urlopen(url,timeout=25) as response,archive.open('wb') as file:
                length=int(response.headers.get('Content-Length','0'));assert not length or length<=16_000_000
                while True:
                    assert time.monotonic()-began<180,'small ZIP bounded transport deadline'
                    chunk=response.read(65536)
                    if not chunk:break
                    received+=len(chunk);assert received<=16_000_000;file.write(chunk)
            assert received==light['size_in_bytes'];digest=hashlib.sha256(archive.read_bytes()).hexdigest()
            if light.get('digest'):assert light['digest']=='sha256:'+digest
            with zipfile.ZipFile(archive) as zipped:
                assert sum(i.file_size for i in zipped.infolist())<=500_000_000
                for item in zipped.infolist():
                    parts=Path(item.filename).parts;assert parts and '..' not in parts and not item.filename.startswith('/') and 'seed-target-and-source.tar.zst' not in item.filename
                zipped.extractall(out/'results')
            result.update(exit=0,bytes=received,zip_sha256=digest,cache_artifacts_not_downloaded=[a for a in meta['artifacts'] if a['name']!=name])
        except Exception as error:result['transport_error']=type(error).__name__+': '+str(error)
        result.update(elapsed_seconds=time.monotonic()-began,ready_at=now());save(out/'small-results-transport.json',result)
        return result
    # Independent light reads share this owner and budget; first cause never waits for ZIP.
    with ThreadPoolExecutor(max_workers=2) as workers:
        cause_future=workers.submit(stdout_cause);download_future=workers.submit(small_results)
        cause=cause_future.result();download=download_future.result()
    assert download['exit']==0,'small artifact transport failed; stdout attribution preserved, never Rust retry'
    dirs=list((out/'results').rglob('ci-evidence'));assert len(dirs)==1;evidence=dirs[0]
    phases=[read(path) for path in sorted(evidence.glob('*.summary.json'))]
    state=read(evidence/'qualification-phase-state.json') if (evidence/'qualification-phase-state.json').exists() else {}
    failure=read(evidence/'failure.json') if (evidence/'failure.json').exists() else None
    native=read(evidence/'native31-cases.json') if (evidence/'native31-cases.json').exists() else None
    budget=read(evidence/'budget-after.json') if (evidence/'budget-after.json').exists() else {}
    receipt={'run_id':run_id,'control_sha':control_sha,'candidate_sha':CRATE_CANDIDATE,'CI_conclusion':record['conclusion'],'state':state,'phases':phases,'failure':failure,
        'native31_cases':native,'watch':watched,'stdout_first_attribution':cause,'small_results_transport':download,'small_raw_receipt_ready_at':now(),
        'budget_after':budget,'cache_artifacts_not_downloaded':download['cache_artifacts_not_downloaded'],'not_optimization_or_Root_PASS':True}
    save(out/'attribution-receipt.json',receipt)
    final=(evidence/'final-cgroup.events').read_text() if (evidence/'final-cgroup.events').exists() else ''
    result={'status':'PASS_TWO_CLEAN_CHECKS_SCOPE' if record['conclusion']=='success' and state.get('apps_check_complete') and state.get('API_alltargets_complete') else 'FAIL_OR_INCOMPLETE_CLEAN_CHECKS_SCOPE',
        'run_id':run_id,'control_sha':control_sha,'candidate_sha':CRATE_CANDIDATE,'apps_check_complete':state.get('apps_check_complete'),'API_alltargets_complete':state.get('API_alltargets_complete'),'frozen_pair_completion':'incomplete','C01_not_run':True,'native_actual_passed':native['passed'] if native else None,
        'native_actual_test_counts':{'passed':native['passed'],'failed':0,'ignored':0} if native else None,'failure':failure,'budget_after':budget,
        'all_command_cleanup_complete':all(r['cleanup_complete'] for r in phases),'final_populated_zero':'populated 0' in final,'no_private_root_or_PG_or_tail':True,
        'clean_check_artifact_files':['original-three-package-check-artifacts.json','API-only-all-targets-check-artifacts.json'],'restoration_net_lines':173,'not_optimization_or_Root_PASS':True}
    save(out/'qualification-summary.json',result)
    (out/'report.md').write_text(result['status']+'\n\n'+json.dumps(result,indent=2)+'\nEvidence: attribution-receipt.json, stdout-phase-attribution.json, small-results-transport.json, results/ci-evidence/*.\nClean check scope only; no behavior/C01 D/optimization/Root or remaining-obligation PASS.\n')
    print(json.dumps({'clean_checks_complete':result,'receipt':str(out/'attribution-receipt.json')}),flush=True)
    return 0


def attribute_api(out,evidence,receipt,run_record):
    from datetime import datetime
    def optional(name,default=None):
        file=evidence/name
        return read(file) if file.exists() else default
    state=optional('qualification-phase-state.json',{});budget=optional('budget-after.json',{});fresh=optional('restored-cargo-fresh.json',{});stage=optional('compiler-stage-summary.json',{});proof=optional('mono-generation-proof.json',{})
    build=next((r for r in receipt['phases'] if r['phase']=='seed-normal-lib-prebuild'),None)
    api_units=[r for r in fresh.get('artifacts',[]) if r['target']=='api_server' and r['kind']==['lib']]
    comparison=None;packed=evidence/'api_server.mono_items.json.gz'
    if packed.exists() and proof.get('generation')=='new-source-bound-output':
        with gzip.open(packed,'rb') as source:raw=source.read(32_000_001)
        assert len(raw)<=32_000_000 and len(raw)==proof['bytes'] and hashlib.sha256(raw).hexdigest()==proof['sha256']
        assert proof['source_sha']==CRATE_CANDIDATE and proof['sha256']!=OLD_API_MONO_SHA256
        (out/'api_server.mono_items.json').write_bytes(raw)
        save(out/'mono-integrity.json',proof)
        task=out.parent
        frozen=read(task/'model/candidates/raw/generic-boundary/w2-w3-mono-aggregation.json')
        tables={};files={}
        for side in ['w2','w3']:
            file=task/Path(frozen['files'][side]['path']).relative_to('tmp/test-governance/issue2818');content=file.read_bytes()
            assert hashlib.sha256(content).hexdigest()==frozen['files'][side]['sha256']
            tables[side]=json.loads(content);files[side]=frozen['files'][side]
        old=task/'ci-w4-api-42603827669/api_server.mono_items.json';content=old.read_bytes();assert hashlib.sha256(content).hexdigest()==OLD_API_MONO_SHA256
        tables['w4']=json.loads(content);files['w4']={'path':str(old),'sha256':OLD_API_MONO_SHA256}
        tables['w5']=json.loads(raw);files['w5']={'path':str(out/'api_server.mono_items.json'),'sha256':proof['sha256'],'source_sha':CRATE_CANDIDATE}
        def total(rows):return {'definitions':len(rows),'instantiations':sum(r['instantiation_count'] for r in rows),'estimate':sum(r['total_estimate'] for r in rows)}
        selectors={
            'old_typed_unary_prepare':lambda n:n.startswith('interface_runtime::invocation::kernel::') and '::prepare_unary' in n,
            'old_typed_unary_execute':lambda n:n.startswith('interface_runtime::invocation::kernel::') and '::execute_unary' in n,
            'old_W4_policy_driver':lambda n:n.startswith('interface_runtime::invocation::policy::'),
            'new_full_unary_source_family':lambda n:n.startswith('interface_runtime::invocation::unary::'),
            'new_private_typed_bridge':lambda n:'TypedUnaryBridge' in n,
            'new_O_only_output_mapper':lambda n:n.startswith('interface_runtime::invocation::unary::map_output'),
            'kernel_all':lambda n:n.startswith('interface_runtime::invocation::kernel::'),
            'await_in_flight':lambda n:n.startswith('interface_runtime::invocation::await_in_flight'),
            'decision_contributions':lambda n:'interface_runtime::decision::AuthorizationContributions' in n or 'interface_runtime::decision::AdmissionContributions' in n,
            'failure_observers':lambda n:'interface_runtime::hook::FailureObservers' in n,
            'canonical_prefix':lambda n:n.startswith('interface_runtime::'),
            'carrier_std_core_alloc':lambda n:n.startswith(('std::','core::','alloc::')),
            'carrier_drop':lambda n:'::drop_in_place' in n,
            'carrier_tokio':lambda n:n.startswith('tokio::')}
        common={name:lambda n,name=name:n.replace("::<'_>",'').startswith('interface_runtime::invocation::unary::'+name) for name in ['start_unary','prepare','reject','execute']}
        selectors.update({'new_common_'+name:select for name,select in common.items()})
        groups={name:{side:total([r for r in rows if select(r['name'])]) for side,rows in tables.items()} for name,select in selectors.items()}
        sums={side:total(rows) for side,rows in tables.items()};assert sums['w2']==frozen['totals']['w2'] and sums['w3']==frozen['totals']['w3']
        bodies={name:[r for r in tables['w5'] if select(r['name'])] for name,select in common.items()}
        comparison={'files':files,'totals':sums,'delta_W5_minus_W4':{key:sums['w5'][key]-sums['w4'][key] for key in sums['w4']},
            'overlapping_source_and_carrier_families':groups,'common_driver_rows':bodies,'common_driver_each_one':{name:bool(rows) and all(r['instantiation_count']==1 for r in rows) for name,rows in bodies.items()},
            'private_bridge_rows':[r for r in tables['w5'] if 'TypedUnaryBridge' in r['name']],
            'O_only_mapper_rows':[r for r in tables['w5'] if selectors['new_O_only_output_mapper'](r['name'])],
            'limits':['Same-source mono collector rows only; not timing, RSS or byte size.','Family views overlap and must not be added twice. Carriers have no exclusive owner attribution.','Typed bridges and O-only mapper remaining instances are reported; no claim that all typed code disappears.','Retained bytes were not measured. No print-type-sizes or other new diagnostics.']}
        save(out/'mono-comparison.json',comparison)
    def ts(t):return datetime.fromisoformat(t.replace('Z','+00:00'))
    job=run_record['jobs'][0];complete=bool(state.get('normal_lib_complete'));valid=complete and len(api_units)==1 and not api_units[0]['profile']['test']
    final=(evidence/'final-cgroup.events').read_text() if (evidence/'final-cgroup.events').exists() else ''
    summary={'status':'PASS_ORDINARY_API_LIB_SCOPE' if valid else 'FAIL_ORDINARY_API_LIB_SCOPE','run_id':receipt['run_id'],'source_sha':CRATE_CANDIDATE,'control_sha':receipt['control_sha'],
        'normal_lib_complete':complete,'actual_test_counts':None,'root_build_phase':build,'successful_api_root_artifact':api_units,
        'fresh_completed':fresh.get('fresh_artifacts'),'rebuilt_completed':fresh.get('rebuilt_artifacts'),'compiler_stages':stage,'mono_generation':proof,
        'mono_comparison_available':comparison is not None,'common_driver_each_one':comparison['common_driver_each_one'] if comparison else None,
        'failure':receipt['failure'],'budget_after':budget,'all_command_cleanup_complete':all(r['cleanup_complete'] for r in receipt['phases']),
        'final_cgroup_populated_zero':'populated 0' in final,'job_elapsed_seconds':(ts(job['completedAt'])-ts(job['startedAt'])).total_seconds(),
        'first_raw_attribution_delay_seconds':(ts(receipt['small_results_ready_at'])-ts(job['completedAt'])).total_seconds(),
        'local_light_download_seconds':receipt['light_download']['elapsed_seconds'],'light_artifact':receipt['light_artifact'],
        'cache_artifacts_not_downloaded':receipt['cache_artifacts_not_downloaded'],'seed_retention':optional('seed-retention.json'),'api_cost_summary':optional('api-cost-summary.json'),
        'tail_deferred':['private24','native7','seed','MCP','application','alltargets','C01'],'retainedbytes_not_measured':True,'no_followon_or_retry':True,'not_behavior_or_Root_or_performance_PASS':True}
    save(out/'api-result-summary.json',summary)
    lines=[summary['status'],'','Run '+str(receipt['run_id'])+' '+run_record['url'],'Source '+CRATE_CANDIDATE+'; control '+receipt['control_sha'],
        'Ordinary API complete '+str(complete)+'; behavior test counts null. Deferred '+json.dumps(summary['tail_deferred'])+'.',
        'Build '+json.dumps(build),'Budget '+json.dumps(budget),'Cleanup '+str(summary['all_command_cleanup_complete'])+'; populated0 '+str(summary['final_cgroup_populated_zero']),
        'Raw delay '+str(summary['first_raw_attribution_delay_seconds'])+'s; direct small download '+str(summary['local_light_download_seconds'])+'s; cache not downloaded.']
    if comparison:
        lines+=['','Common driver instances: '+json.dumps(comparison['common_driver_each_one']),
            '| Source family | W2 defs/instances/estimate | W3 | W4 | W5 |','| --- | --- | --- | --- | --- |']
        for name,group in {'TOTAL':comparison['totals'],**comparison['overlapping_source_and_carrier_families']}.items():lines.append('| '+name+' | '+' | '.join(' / '.join(str(group[side][k]) for k in ['definitions','instantiations','estimate']) for side in ['w2','w3','w4','w5'])+' |')
    lines+=['','Mono estimates are not RSS/time or exclusive ownership. No retainedbytes measurement. Preserve failure/cache/cleanup and stop failed configuration; no auto retry/tail.','Evidence: attribution-receipt.json, results/ci-evidence/*, mono-integrity.json, mono-comparison.json, api-result-summary.json.']
    (out/'report.md').write_text('\n'.join(lines)+'\n')
    print(json.dumps({'api_attribution':{k:summary[k] for k in ['status','run_id','normal_lib_complete','fresh_completed','rebuilt_completed','common_driver_each_one','first_raw_attribution_delay_seconds']},'report':str(out/'report.md')}),flush=True)


def main():
    candidate=os.environ['CANDIDATE_SHA'];assert candidate==SEED_SHA==CRATE_CANDIDATE=='7bb249e47d29dce87a291677884bd42962aa2046'
    EVIDENCE.mkdir(parents=True,exist_ok=False);TEMP.mkdir(parents=True,exist_ok=False)
    source,target=TEMP/'seed-source',TEMP/'target-seed'
    metadata={'source_sha':candidate,'scope':'original three-package clean check then API-only all-targets clean check','apps_check_complete':False,'API_alltargets_complete':False,
        'active_phase':'prepare','no_PG':True,'no_tests_executed':True,'no_C01_injection_projection_or_literal':True,'frozen_pair_completion':'incomplete','resumed_from_run':PRIOR_RUN,'W6_not_included':True}
    initial=budget_snapshot();save(EVIDENCE/'budget-before.json',initial)
    assert int(initial['memory.max'])==8_000_000_000 and initial['memory.high']=='max' and initial['memory.swap.max']=='0'
    q,p=map(int,initial['cpu.max'].split());assert q/p<=os.cpu_count()*.6+.001
    assert TEMP==Path('/home/runner/work/_temp/issue2818') and capture(['rustc','--version']).startswith('rustc 1.99.1 ')
    assert not any(os.environ.get(k) for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS'])
    os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=candidate
    save(EVIDENCE/'environment.json',{'source_sha':candidate,'rustc':capture(['rustc','-vV']),'cargo':capture(['cargo','-vV']),'scope':metadata['scope'],'host_cpu_count':os.cpu_count(),
        'profile':'original default dev/check, no explicit profile override','features':'original manifest resolution, no added feature flags','no_PG':True,'C01':'not executed; denominator/pair unchanged',
        'cache_scope':'restored test-profile builds do not guarantee check-mode metadata Fresh; record all artifacts and added costs'})
    try:
        archive,incoming=download_retained()
        run('checkout-archive-source',['git','worktree','add','--detach',str(source),candidate],seconds=120)
        run('restore-retained-source-target',['tar','-I','zstd -T1','-xf',str(archive),'--no-same-owner','-C',str(TEMP)],seconds=180)
        assert capture(['git','rev-parse','HEAD'],source)==candidate;capture(['git','diff','--exit-code','HEAD','--'],source)
        assert target.is_dir() and any(target.rglob('*.rlib'))
        hashes={path:hashlib.sha256((source/path).read_bytes()).hexdigest() for path in WITHDRAWAL_AFTER_HASHES};assert hashes==WITHDRAWAL_AFTER_HASHES
        ir={path:hashlib.sha256((source/path).read_bytes()).hexdigest() if (source/path).is_file() else None for path in API_SOURCE_SHA256};assert ir==API_SOURCE_SHA256
        c01=source/'api/apps/api-server/tests/http/api_identity.rs';c01_bytes=c01.read_bytes();assert b'ISSUE2818_UNSELECTED_ROOT_TYPE_NEGATIVE' not in c01_bytes
        save(EVIDENCE/'source-identity.json',{'actual_sha':candidate,'clean_tracked_source':True,'source_hashes':hashes,'IR_hashes':ir,'no_postrestore_checkout':True,
            'C01_path':str(c01),'C01_clean_sha256':hashlib.sha256(c01_bytes).hexdigest(),'marker_not_injected':True,'W6_not_included':True})
        run('registry-prefetch',['cargo','fetch','--locked','--manifest-path','api/Cargo.toml'],cwd=source,seconds=900)
        meta=run('cargo-metadata',['cargo','metadata','--no-deps','--format-version','1','--manifest-path','api/Cargo.toml'],cwd=source,seconds=60)
        cargo_meta=json.loads(Path(meta['stdout_log']).read_text());packages={p['name']:p for p in cargo_meta['packages']}
        save(EVIDENCE/'requested-target-contract.json',{'packages':{name:{'id':packages[name]['id'],'targets':packages[name]['targets'],'default_features':packages[name]['features'].get('default',[])} for name in ['control-plane','api-server','control-plane-postgres-tests']},
            'commands':CLEAN_CHECK_COMMANDS,'no_CP_alltargets':True,'extra_observation_arg':'--message-format=json only; no rustc diagnostic flags'})
        env={**os.environ,'CARGO_TARGET_DIR':str(target),'CARGO_BUILD_JOBS':'1','CARGO_INCREMENTAL':'0','CARGO_NET_OFFLINE':'true','CARGO_TERM_COLOR':'never','TERM':'dumb'}
        for key in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:env.pop(key,None)
        save(EVIDENCE/'effective-cargo-env.json',{k:env.get(k) for k in ['CARGO_TARGET_DIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_NET_OFFLINE','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']})
        for index,original in enumerate(CLEAN_CHECK_COMMANDS):
            label='original-three-package-check' if index==0 else 'API-only-all-targets-check';metadata['active_phase']=label
            argv=[*original,'--message-format=json']
            row=run(label,argv,cwd=source/'api',env=env,heavy=True,seconds=600)
            artifacts=[]
            for line in Path(row['merged_log']).open(errors='replace'):
                try:item=json.loads(line)
                except json.JSONDecodeError:continue
                if item.get('reason')=='compiler-artifact':artifacts.append(item)
            assert artifacts,'successful check without completed artifacts'
            api_id=packages['api-server']['id'];api_units=[r for r in artifacts if r['package_id']==api_id]
            if index==0:
                default_units={name:[r for r in artifacts if r['package_id']==packages[name]['id'] and r['target']['kind']!=['custom-build']] for name in ['control-plane','api-server','control-plane-postgres-tests']}
                expected_normal={name for name in default_units if any(t['kind'] in [['lib'],['bin']] for t in packages[name]['targets'])}
                assert all(default_units[name] for name in expected_normal),'requested existing normal roots missing'
                save(EVIDENCE/'three-package-default-selection.json',{'expected_normal_package_names':sorted(expected_normal),'actual_units':default_units,'packages_without_normal_targets':[name for name in default_units if name not in expected_normal],'scope':'exact original command; do not invent full test coverage for a package with only integration targets'})
                metadata['apps_check_complete']=True
            else:
                identity=[r for r in api_units if r['target']['name']=='api_identity' and r['target']['kind']==['test']]
                assert len(identity)==1 and Path(identity[0]['target']['src_path'])==c01
                assert not any(r['package_id'] in [packages['control-plane']['id'],packages['control-plane-postgres-tests']['id']] and r['profile']['test'] for r in artifacts),'CP all-targets scope expansion'
                metadata['API_alltargets_complete']=True
            save(EVIDENCE/(label+'-artifacts.json'),{'source_sha':candidate,'original_argv':original,'executed_argv':argv,'fresh':sum(r['fresh'] for r in artifacts),
                'rebuilt':sum(not r['fresh'] for r in artifacts),'api_units':api_units,'all_artifacts':artifacts,'actual_test_counts':None,'default_check_profile_and_features':'see actual artifact profile/features'})
        assert c01.read_bytes()==c01_bytes;capture(['git','diff','--exit-code','HEAD','--'],source)
        save(EVIDENCE/'status.json',{'status':'TWO_ORIGINAL_CLEAN_CHECKS_QUALIFIED','source_sha':candidate,'apps_check_complete':True,'API_alltargets_complete':True,
            'actual_test_counts':None,'frozen_pair_completion':'incomplete','C01_not_run':True,'not_Root_or_performance_PASS':True})
        return 0
    finally:
        save(EVIDENCE/'qualification-phase-state.json',metadata);retain_seed_cache(source,target,metadata)
        save(EVIDENCE/'clean-check-cost-summary.json',{'commands':RESULTS,'total_measured_command_seconds':sum(r['wall_seconds'] for r in RESULTS),
            'scope':'two clean check modes only; restore/Fresh/new metadata/alltargets/failure/archive costs retained, no C01 or projected-old D evidence'})




if __name__=='__main__':
    if len(sys.argv)>1 and sys.argv[1] in ('--help','-h'):
        print('Only original three-package and API-only all-targets clean checks. Local owner: follow-clean-checks RUN_ID OUT_DIR CONTROL_SHA. --self-check validates source/transport/artifact routing without execution.')
        sys.exit(0)
    if len(sys.argv)>1 and sys.argv[1]=='--self-check':
        import yaml
        workflow=yaml.safe_load(SELF.with_suffix('.yml').read_text())
        uploads=[step['with']['name'] for job in workflow['jobs'].values() for step in job['steps'] if str(step.get('uses','')).startswith('actions/upload-artifact@')]
        assert uploads[0].startswith(RESULTS_PREFIX) and uploads[1].startswith('issue2818-clean-checks-cache-')
        assert len(CLEAN_CHECK_COMMANDS)==2 and CLEAN_CHECK_COMMANDS[0]==['cargo','check','--package','control-plane','--package','api-server','--package','control-plane-postgres-tests','--jobs','1']
        assert CLEAN_CHECK_COMMANDS[1]==['cargo','check','--package','api-server','--all-targets','--jobs','1']
        assert len(WITHDRAWAL_ALLOWED)==20
        assert set(NATIVE31_NAMES)==set(W2_NATIVE_SEVEN+[row['full_test_name'] for row in WITHDRAWAL_IDENTITY_MAP])
        assert set(WITHDRAWAL_ALLOWED)==set(WITHDRAWAL_AFTER_HASHES)==set(WITHDRAWAL_BEFORE_HASHES)
        assert SEED_SHA=='7bb249e47d29dce87a291677884bd42962aa2046' and CRATE_CANDIDATE=='7bb249e47d29dce87a291677884bd42962aa2046'
        print(json.dumps({'self_check':'PASS','artifact_prefix_single_route':RESULTS_PREFIX,'clean_command_count':2,'C01_executed':False,'source_paths':20,'runtime_or_network_actions':False}))
        sys.exit(0)
    if len(sys.argv)>1 and sys.argv[1]=='follow-clean-checks':
        sys.exit(follow_withdrawal(int(sys.argv[2]),sys.argv[3],sys.argv[4]))
    if len(sys.argv)>1 and sys.argv[1]=='exec':
        sys.exit(stream_exec(sys.argv[3:],sys.argv[2]))
    try:
        sys.exit(main())
    except Exception as error:
        if EVIDENCE.exists():save(EVIDENCE/'failure.json',{'kind':type(error).__name__,'message':str(error),'classification':'inspect first failing same-source raw receipt; not optimization or behavior PASS'})
        raise
    finally:
        if EVIDENCE.exists():
            try:save(EVIDENCE/'budget-after.json',budget_snapshot())
            except (OSError,KeyError):pass
