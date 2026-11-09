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

SEED_SHA = 'a9e85b53bb385349896a16eeaa32238093248626'
PRIOR_RUN = '42572052197'
PRIOR_NAME = 'issue2818-seed-owner-42572052197-1'
PRIOR_ARTIFACT_ID = 13654072605
ARCHIVE_SHA = '83177736363129f08913e94fce5e2d050e9d772c7b1a634bcdbdcb26f93b6a05'
ARCHIVE_BYTES = 680288609
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
    paths = capture(['git','ls-files','-z'],root).split('\0')
    # Source, fixtures and all configuration stay exact; only intended JS selectors may differ.
    allowed = {'scripts/node/verify/backend.js','scripts/node/verify/backend-targets.js',
               'scripts/node/foundation-contracts/core.js',
               'scripts/node/foundation-contracts/_tests/core.test.js',
               'scripts/node/verify-backend/_tests/cli.test.js',
               'scripts/node/verify/_tests/history-maintenance.test.js',
               'scripts/shell/resource-job-run.py','scripts/shell/_tests/resource-job-run.test.py'}
    selected = [p for p in paths if p not in allowed]
    hashes = {p:hashlib.sha256((root/p).read_bytes()).hexdigest()
              for p in selected if (root/p).is_file()}
    return hashes

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
    prior_plan=read(evidence/'seed-source-plan.json')
    commands=read(evidence/'commands.json')
    old_build=next(c for c in commands if c['label']=='seed-normal-lib-prebuild')
    original_prebuild=old_build['argv']
    prebuild=[arg for arg in original_prebuild if not arg.startswith('--message-format=')]
    assert prebuild==['cargo','build','--profile','test','--package','api-server',
        '--features','test-support','--lib','--jobs','1']
    assert prior_plan['runtime']['backend']['cargoJobs']==2, 'incoming source runtime must remain observed jobs=2'
    os.environ['ISSUE2818_FROZEN_RUNTIME']=json.dumps(prior_plan['runtime'])
    old_samples=evidence/Path(old_build['merged_log']).name.replace('.combined.log','.native-samples.jsonl')
    run('summarize-prior-native',[sys.executable,str(SELF),'prior-summary',str(old_samples),
        str(EVIDENCE/'prior-native-summary.json')],seconds=90)
    save(EVIDENCE/'cache-resume.json',{'source_run':PRIOR_RUN,'artifact':PRIOR_NAME,
        'archive_sha256':ARCHIVE_SHA,'archive_bytes':ARCHIVE_BYTES,'source_sha':SEED_SHA,
        'runtime':prior_plan['runtime'],'original_prebuild_argv':original_prebuild,
        'cache_state':'restored partial a9 target from deadline with OOM evidence; not cold','memory_high':'max',
        'incoming_jobs':1,'diagnostic_jobs':1,'compiler_stage_flags':'root-only temporary RUSTC_WRAPPER: -Ztime-passes -Ztime-passes-format=json',
        'different_condition':'W2 19-file owner change; old archive remains a9; no cold or PR-topology performance claim'})
    return archive,evidence,prior_plan,prebuild

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

ROOT_WRAPPER = "#!/usr/bin/env python3\nimport json,os,re,sys,time\nfrom pathlib import Path\nreal,args=sys.argv[1],sys.argv[2:]\ndef value(flag):\n    if flag in args:\n        return args[args.index(flag)+1]\n    return next((x.split('=',1)[1] for x in args if x.startswith(flag+'=')),None)\nmatch=(value('--crate-name')=='api_server' and value('--crate-type')=='lib'\n       and '--test' not in args and any(Path(x).name=='lib_root.rs' for x in args))\nenv=dict(os.environ)\neffective=list(args)\nif match:\n    mode=env.get('ISSUE2818_RUSTC_MODE','diagnostic')\n    if mode=='diagnostic':\n        effective+=['-Ztime-passes','-Ztime-passes-format=json']\n        if env.get('ISSUE2818_MONO_STATS')=='1':\n            effective+=['-Zdump-mono-stats','-Zdump-mono-stats-format=json']\n        env['RUSTC_BOOTSTRAP']='api_server'\n    auth=re.search(r'--jobserver-(?:auth|fds)=(\\d+),(\\d+)',env.get('CARGO_MAKEFLAGS',''))\n    fds={}\n    for fd in (map(int,auth.groups()) if auth else []):\n        try:\n            os.fstat(fd)\n            fds[str(fd)]='open'\n        except OSError:\n            fds[str(fd)]='closed'\n    row={'pid':os.getpid(),'monotonic':time.monotonic(),'mode':mode,'probe':'-Zhelp' in args,\n         'original_argv':[real,*args],'effective_argv':[real,*effective],\n         'bootstrap_for_this_exec':env.get('RUSTC_BOOTSTRAP'),'jobserver_fds':fds}\n    with open(env['ISSUE2818_ROOT_LOG'],'a') as log:\n        log.write(json.dumps(row)+'\\n')\n        log.flush()\n    if mode=='observe-only':\n        os._exit(86)\nos.execve(real,[real,*effective],env)\n"

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
        'last_completed_pass':completed[-1] if completed else None,'completed_passes':completed,
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

def main():
    # Lowest-cost next evidence: one real database-free seed owner, not another full W.
    EVIDENCE.mkdir(parents=True,exist_ok=False)
    TEMP.mkdir(parents=True,exist_ok=False)
    metadata={'source_sha':os.environ['CANDIDATE_SHA'],'normal_lib_complete':False,'seed_queries_complete':False,
              'resumed_from_run':PRIOR_RUN,'cache_start':'restored-partial-target',
              'diagnostic_jobs':1,'default_ordinary_lib_fresh_verified':False,
              'root_only_diagnostics':['-Ztime-passes','-Ztime-passes-format=json']}
    assert metadata['source_sha']==W2_CANDIDATE and W2_CANDIDATE!=SEED_SHA
    assert TEMP==Path('/home/runner/work/_temp/issue2818'), 'restore paths must stay exact'
    source=TEMP/'seed-source'
    target=TEMP/'target-seed'
    initial=budget_snapshot()
    save(EVIDENCE/'budget-before.json',initial)
    assert int(initial['memory.max'])==8_000_000_000 and initial['memory.swap.max']=='0'
    assert initial['memory.high']=='max', 'no extra MemoryHigh policy in this diagnostic window'
    quota,period=map(int,initial['cpu.max'].split())
    assert quota/period<=os.cpu_count()*.6+.001
    assert capture(['rustc','--version']).startswith('rustc 1.99.1 '), 'exact compiler pin required'
    save(EVIDENCE/'environment.json',{'rustc':capture(['rustc','-vV']),'cargo':capture(['cargo','-vV']),
         'host_cpu_count':os.cpu_count(),'source_sha':metadata['source_sha'],'postgres':False,
         'phase':'W2 first candidate: ordinary lib plus private owner/native seven/seed total feasibility',
         'cache_start':'restored-partial, not cold','memory_high':'max','extra_G0_policy':False})
    try:
        archive,prior_evidence,prior_plan,prior_build=download_retained()
        source,data,cargo_meta=prepare('seed-source',SEED_SHA,archive)
        assert data==prior_plan, 'archive source command plan changed'
        assert frozen(source)==read(prior_evidence/'seed-source-source-hashes.json'), 'restored a9 source/config/fixture hash mismatch'
        promote_w2(source,metadata['source_sha'],prior_evidence)
        data=plan(source)
        assert data==prior_plan, 'unrelated seed/app commands changed in W2'
        save(EVIDENCE/'restored-source-validation.json',{'source_sha':metadata['source_sha'],
            'tracked_git_diff':'empty','source_hashes_match':True,'native_mtime_restore':True,
            'source_path':str(source),'target_path':str(target),'target_nonempty':True})
        seed_data={**data,'tests':data['tests'][:2],'checks':[]}
        env=command_env(data,seed_data['tests'][0],target)
        run('registry-prefetch',['cargo','fetch','--locked','--manifest-path','api/Cargo.toml'],cwd=source,seconds=900)
        graph_env={**env,'RUSTC_BOOTSTRAP':'1'}
        original=seed_data['tests'][0]['args'];stop=original.index('--') if '--' in original else len(original)
        qgraph=run('seed-query-unit-graph',['cargo',*original[:stop],'-Z','unstable-options','--unit-graph','--no-run'],cwd=source/'api',env=graph_env,seconds=60)
        build=list(prior_build)
        build[build.index('--jobs')+1]='1'
        bgraph=run('seed-lib-unit-graph',[*build,'-Z','unstable-options','--unit-graph'],cwd=source/'api',env={**graph_env,'CARGO_BUILD_JOBS':'1'},seconds=60)
        q=json.loads(Path(qgraph['stdout_log']).read_text());b=json.loads(Path(bgraph['stdout_log']).read_text())
        def sig(u):
            return json.dumps({k:u[k] for k in ('pkg_id','target','profile','platform','mode','features')},sort_keys=True)
        def closure(g,i):
            seen=set();todo=[i]
            while todo:
                k=todo.pop()
                if k in seen:continue
                seen.add(k);todo.extend(d['index'] for d in g['units'][k]['dependencies'])
            return {sig(g['units'][k]) for k in seen}
        r=b['roots'][0];matches=[i for i,u in enumerate(q['units']) if sig(u)==sig(b['units'][r])]
        assert len(matches)==1 and closure(q,matches[0])==closure(b,r), 'prebuild not exact seed dependency'
        assert len(closure(b,r))==548, 'the frozen ordinary-lib dependency graph must remain 548 units'
        save(EVIDENCE/'seed-prebuild-compatibility.json',{'exact_recursive_unit_equality':True,'units':len(closure(b,r)),
             'compiler':capture(['rustc','--version']),'purpose':'same ordinary-lib build profile/features and 548 units; jobs2 incoming to jobs1 diagnostic; root-only stage flags are not a runtime equivalence proof'})
        diagnostic=install_stage_wrapper(source,env)
        run('seed-normal-lib-prebuild',[*build,'--message-format=json'],cwd=source/'api',env=diagnostic,heavy=True,seconds=600)
        cargo_fresh_summary()
        fresh=read(EVIDENCE/'restored-cargo-fresh.json')
        assert fresh['fresh_artifacts']>=509, 'expected 509 cached dependency artifacts; no cold fallback'
        assert not [r for r in fresh['artifacts'] if not r['fresh'] and r['target']!='api_server'], 'dependency rebuilt unexpectedly; stop before seed'
        assert stage_summary()['diagnostic_root_execs']==1, 'expected exactly one ordinary API lib diagnostic compilation'
        metadata['normal_lib_complete']=True
        # Same temporary wrapper in observe-only mode refuses any hidden second ordinary-lib compile.
        observe={**diagnostic,'ISSUE2818_RUSTC_MODE':'observe-only'}
        default=run('default-lib-freshness-check',[*build,'--message-format=json'],
            cwd=source/'api',env=observe,heavy=True,seconds=90)
        default_artifacts=[]
        for line in Path(default['merged_log']).read_text().splitlines():
            try:
                row=json.loads(line.removeprefix('time: '))
            except json.JSONDecodeError:
                continue
            if row.get('reason')=='compiler-artifact' and row['target']['name']=='api_server' and row['target']['kind']==['lib']:
                default_artifacts.append(row)
        assert default_artifacts and all(r['fresh'] for r in default_artifacts), 'default API lib is not Cargo Fresh; no automatic recompile'
        assert stage_summary()['default_root_recompile_blocked']==0
        metadata['default_ordinary_lib_fresh_verified']=True
        save(EVIDENCE/'default-lib-freshness.json',{'cargo_reports_fresh':True,
            'root_compiler_was_not_invoked':True,'diagnostic_flags_not_reapplied':True,
            'not_a_blanket_cache_equivalence_or_performance_claim':True})
        whitebox_names=[r['full_test_name'] for r in W2_MAP]
        assert len(whitebox_names)==24 and len(set(whitebox_names))==24 and not any(r['ignored'] for r in W2_MAP)
        private_list,private_actual=execute_w2_owner(source,data,cargo_meta,target,diagnostic,
            'api_private_routes','console_route_assembly',whitebox_names,'w2-private')
        membership=[]
        for selector in ['migrated_assembly_contains_every_console_router_owner_assembly',
                         'console_route_assembly','complete_operation_specifications_match_the_production_route_set']:
            selected=[r for r in private_list if selector in r[1]]
            assert selected and all(any(a[0]==r[0] and a[1]==r[1] and a[2]=='ok' for a in private_actual) for r in selected)
            membership.append({'filter':selector,'target':'api_private_routes','compiled_list_members':selected,
                               'nonzero_actual_source':'the single 24-case execution; no replay'})
        save(EVIDENCE/'w2-historical-caller-membership.json',membership)
        execute_w2_owner(source,data,cargo_meta,target,diagnostic,'console_core_registry_tests','',
            W2_NATIVE_SEVEN,'w2-native')
        executions=[]
        for index,command in enumerate(seed_data['tests']):
            executions.append(run('seed-query-'+str(index),[command['command'],*command['args']],
                 cwd=source/command['cwd'],env={**command_env(data,command,target),
                     'RUSTC_WRAPPER':diagnostic['RUSTC_WRAPPER'],'ISSUE2818_RUSTC_MODE':'observe-only',
                     'ISSUE2818_ROOT_LOG':diagnostic['ISSUE2818_ROOT_LOG']},heavy=True,seconds=600))
        metadata['seed_queries_complete']=True
        # Listings use the same observe-only guard; no root flags or silent ordinary-lib rebuild.
        os.environ.update(RUSTC_WRAPPER=diagnostic['RUSTC_WRAPPER'],ISSUE2818_RUSTC_MODE='observe-only',
                          ISSUE2818_ROOT_LOG=diagnostic['ISSUE2818_ROOT_LOG'])
        discover('candidate-seed',source,seed_data,cargo_meta,target,executions)
        save(EVIDENCE/'status.json',{'status':'W2_MINIMAL_CANDIDATE_EXECUTED','source_sha':metadata['source_sha'],
             'not_final_QA':True,'not_full_W_or_performance_PASS':True,'remaining':'valid old runtime comparison, MCP/application, known D, additional API all-targets check cost, formal job-boundary A/B; no Root AC PASS'})
        return 0
    finally:
        cargo_fresh_summary()
        stage_summary()
        collect_mono_files(source,target)
        retain_seed_cache(source,target,metadata)
        w2_cost_summary()

if __name__=='__main__':
    if len(sys.argv)>1 and sys.argv[1]=='prior-summary':
        summarize_prior(sys.argv[2],sys.argv[3])
        sys.exit(0)
    if len(sys.argv)>1 and sys.argv[1]=='exec':
        sys.exit(stream_exec(sys.argv[3:],sys.argv[2]))
    try:
        sys.exit(main())
    except Exception as error:
        if EVIDENCE.exists():
            save(EVIDENCE/'failure.json',{'kind':type(error).__name__,'message':str(error),
                 'classification':'invalid baseline / source proof / environment / resource; inspect first failing raw receipt; never optimization PASS'})
        raise
    finally:
        if EVIDENCE.exists():
            try:
                save(EVIDENCE/'budget-after.json',budget_snapshot())
            except (OSError,KeyError):
                pass
