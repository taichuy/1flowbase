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

SEED_SHA = 'ca19501827fdeac86b9f5d9fcba767715220f3dd'
PRIOR_RUN = '42600960280'
PRIOR_NAME = 'issue2818-w4-cache-42600960280-1'
PRIOR_ARTIFACT_ID = 13665767849
ARCHIVE_SHA = '7717f445653790701e6d48f091018c388e05e2f80de9b89997d0849dfedcf081'
ARCHIVE_BYTES = 747218241
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
    save(EVIDENCE/'cache-resume.json',{'source_run':PRIOR_RUN,'artifact':PRIOR_NAME,
        'archive_sha256':ARCHIVE_SHA,'archive_bytes':ARCHIVE_BYTES,'archive_source_sha':SEED_SHA,
        'cache_state':'retained qualified W4 ca195 source/mtime/partial target; no cold fallback',
        'scope':'W4 ordinary API lib qualification; prior focused crate cases not rerun'})
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

OLD_API_MONO_SHA256 = '75ccff8f0dc1a1d9a7bb3a25e683b46e7d29fdb2b2d621eb81260aa98ef9151c'
API_SOURCE_SHA256 = {'api/crates/interface-runtime/src/invocation/kernel.rs': '3f9f2993b19e78adfe5ecb1816e47e3be6b29a1211fa5647d2b2c4ad455479b4', 'api/crates/interface-runtime/src/invocation.rs': 'd12943da62d66771215c1126c78224b27fa21d4b8cb9ec5cafe672d73df20810', 'api/crates/interface-runtime/src/invocation/policy.rs': '9aedf251bf8e1b94a0e4c9d2cb01a070cca430bedb3d9b94e1668789a1d71fe1', 'api/crates/interface-runtime/src/decision.rs': 'b1b06844bc2499c2622b817aac9d4c98908ad6debc906ec9e0119e1d51bab823', 'api/crates/interface-runtime/src/hook.rs': '00b6e24ee82e170c981723476855f0f1a43fdc84fdaffcccf4a0f4801294179a', 'api/crates/interface-runtime/src/_tests/review_remediation_tests/decision_order.rs': '415594bd8ad4c7b546a0bdbbdf06748864116cb3b1c1b09b8084ec5e3522ee2f'}

CRATE_CANDIDATE = 'ca19501827fdeac86b9f5d9fcba767715220f3dd'
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

def follow_w4(run_id,out_dir,control_sha,api=False):
    # One authorized local lifecycle: watch, bounded light download, deterministic receipt.
    from datetime import datetime,timezone
    out=Path(out_dir).resolve();out.mkdir(parents=True,exist_ok=True)
    def now():return datetime.now(timezone.utc).isoformat()
    def command(argv,file,seconds):
        start=time.monotonic()
        with (out/file).open('wb') as log:
            result=subprocess.run(argv,stdout=log,stderr=subprocess.STDOUT,timeout=seconds)
        return {'argv':argv,'exit':result.returncode,'elapsed_seconds':time.monotonic()-start,'log':str(out/file)}
    watched=command(['gh','run','watch',str(run_id),'--interval','30','--exit-status'],'watch.log',2400)
    after_watch=now()
    raw=subprocess.check_output(['gh','run','view',str(run_id),'--json',
        'databaseId,status,conclusion,headSha,url,attempt,jobs'],text=True,timeout=45)
    run=json.loads(raw);save(out/'run-final.json',run);save(out/'jobs.json',run['jobs'])
    assert run['headSha']==control_sha and run['status']=='completed','watch transport failure is not a final CI result'
    meta=json.loads(subprocess.check_output(['gh','api','repos/taichuy/1flowbase/actions/runs/'+str(run_id)+'/artifacts'],text=True,timeout=45))
    save(out/'artifact-meta.json',meta)
    name=('issue2818-w4-api-results-' if api else 'issue2818-w4-results-')+str(run_id)+'-'+str(run['attempt'])
    light=next(a for a in meta['artifacts'] if a['name']==name)
    assert not light['expired'] and light['size_in_bytes']<=16_000_000,'bounded small-results guard; never download cache artifact'
    fetched=command(['gh','run','download',str(run_id),'--name',name,'--dir',str(out/'results')],'download.log',180)
    assert fetched['exit']==0,'light transport failed; do not confuse with Rust failure'
    matches=list((out/'results').rglob('ci-evidence'));assert len(matches)==1
    evidence=matches[0]
    phases=[]
    for path in sorted(evidence.glob('*.summary.json')):
        phases.append(read(path))
    state=read(evidence/'qualification-phase-state.json') if (evidence/'qualification-phase-state.json').exists() else {}
    cases=[]
    for side in ['baseline','candidate']:
        for index in range(3):
            prefix=side+'-q'+str(index)
            path=evidence/(prefix+'-crate-test-cases.json')
            if path.exists():
                record=read(path)
                cases.append({'side':side,'query':index,'filter':record.get('filter'),
                    'passed':record['passed'],'failed':record['failed'],
                    'ignored':sum(r[3] for r in record['list_with_ignored']),
                    'test_artifact_fresh':record.get('test_artifact_fresh'),'raw_json':str(path)})
    failure=read(evidence/'failure.json') if (evidence/'failure.json').exists() else None
    receipt={'run_id':run_id,'control_sha':control_sha,'candidate_sha':CRATE_CANDIDATE,
        'CI_conclusion':run['conclusion'],'source_state':state,'case_groups':cases,'phases':phases,
        'failure':failure,'watch_finished_at':after_watch,'small_results_ready_at':now(),
        'watch':watched,'light_download':fetched,'light_artifact':light,
        'cache_artifacts_not_downloaded':[a for a in meta['artifacts'] if a['name']!=name],
        'not_API_or_W3_78_retest_or_performance_PASS':True}
    save(out/'attribution-receipt.json',receipt)
    if api:
        try:
            attribute_api(out,evidence,receipt,run)
        except Exception as error:
            save(out/'attribution-error.json',{'kind':type(error).__name__,'message':str(error),'raw_receipt':'attribution-receipt.json','CI_conclusion':run['conclusion']})
            (out/'report.md').write_text('Attribution incomplete: '+type(error).__name__+': '+str(error)+'\nRaw small results and attribution-receipt.json are preserved. Do not classify as Rust or behavior PASS. No rerun or cache download.\n')
            raise
    print(json.dumps({'run_id':run_id,'conclusion':run['conclusion'],'candidate':CRATE_CANDIDATE,
        'groups':cases,'failure':failure,'receipt':str(out/'attribution-receipt.json')}),flush=True)
    return 0

def attribute_api(out,evidence,receipt,run_record):
    # Local, deterministic attribution of the downloaded small result; never fetch cache.
    from datetime import datetime
    def loaded(name,default=None):
        path=evidence/name
        return read(path) if path.exists() else default
    state=loaded('qualification-phase-state.json',{})
    root_stage=loaded('compiler-stage-summary.json',{})
    fresh=loaded('restored-cargo-fresh.json',{})
    generation=loaded('mono-generation-proof.json',{})
    budget=loaded('budget-after.json',{})
    phases=receipt['phases']
    build=next((r for r in phases if r['phase']=='seed-normal-lib-prebuild'),None)
    roots=[r for r in fresh.get('artifacts',[]) if r['target']=='api_server' and r['kind']==['lib']]
    native=None;comparison=None
    packed=evidence/'api_server.mono_items.json.gz'
    if packed.exists() and generation.get('generation')=='new-source-bound-output':
        assert generation.get('generation')=='new-source-bound-output'
        assert generation['source_sha']==CRATE_CANDIDATE
        with gzip.open(packed,'rb') as stream:
            raw=stream.read(32_000_001)
        assert len(raw)<=32_000_000 and len(raw)==generation['bytes']
        digest=hashlib.sha256(raw).hexdigest()
        assert digest==generation['sha256'] and digest!=OLD_API_MONO_SHA256
        native=json.loads(raw)
        (out/'api_server.mono_items.json').write_bytes(raw)
        save(out/'mono-integrity.json',{'source_sha':CRATE_CANDIDATE,'bytes':len(raw),'sha256':digest,
            'mtime_ns':generation['mtime_ns'],'build_started_wall_ns':generation['api_build_started_wall_ns'],
            'old_mono_removed':loaded('removed-stale-mono.json'),'compressed_raw_file':str(packed)})
        task_root=out.parent
        frozen=read(task_root/'model/candidates/raw/generic-boundary/w2-w3-mono-aggregation.json')
        tables={}
        files={}
        for key in ['w2','w3']:
            old=task_root/Path(frozen['files'][key]['path']).relative_to('tmp/test-governance/issue2818')
            content=old.read_bytes()
            assert hashlib.sha256(content).hexdigest()==frozen['files'][key]['sha256']
            tables[key]=json.loads(content);files[key]=frozen['files'][key]
        tables['w4']=native
        files['w4']={'path':str(out/'api_server.mono_items.json'),'sha256':digest,'bytes':len(raw),'source_sha':CRATE_CANDIDATE}
        def total(rows):
            return {'definitions':len(rows),'instantiations':sum(r['instantiation_count'] for r in rows),
                'estimate':sum(r['total_estimate'] for r in rows)}
        def unary(n):return n.startswith('interface_runtime::invocation::kernel::') and '::prepare_unary' in n
        def decision_run(n):return n.startswith('interface_runtime::TypedInterface') and ('AuthorizationPlan::' in n.replace('<I, O>','') or 'AdmissionPlan::' in n.replace('<I, O>','')) and '::run' in n
        def failure(n):return n.startswith('interface_runtime::TypedInterfaceHookPlan') and '::run_failure' in n
        heavy={
            'InvocationPolicy::run':lambda n:'interface_runtime::invocation::policy::InvocationPolicy::run' in n,
            'InvocationPolicy::reject':lambda n:'interface_runtime::invocation::policy::InvocationPolicy::reject' in n,
            'AuthorizationContributions::run':lambda n:'interface_runtime::decision::AuthorizationContributions::run' in n,
            'AdmissionContributions::run':lambda n:'interface_runtime::decision::AdmissionContributions::run' in n,
            'FailureObservers::run':lambda n:'interface_runtime::hook::FailureObservers::run' in n}
        def common(n):return any(fn(n) for fn in heavy.values())
        families={
            'unary_preparation':unary,
            'decision_run_typed_shells':decision_run,
            'failure_run_typed_shells':failure,
            'new_common_heavy_bodies':common,
            'whole_changed_common_boundary':lambda n:unary(n) or decision_run(n) or failure(n) or common(n),
            'kernel_all':lambda n:n.startswith('interface_runtime::invocation::kernel::'),
            'await_in_flight':lambda n:n.startswith('interface_runtime::invocation::await_in_flight'),
            'typed_hook_plan_all':lambda n:n.startswith('interface_runtime::TypedInterfaceHookPlan'),
            'typed_decision_plan_all':lambda n:n.startswith(('interface_runtime::TypedInterfaceAuthorizationPlan','interface_runtime::TypedInterfaceAdmissionPlan')),
            'canonical_prefix':lambda n:n.startswith('interface_runtime::'),
            'carrier_std_core_alloc':lambda n:n.startswith(('std::','core::','alloc::')),
            'carrier_drop_in_place':lambda n:'::drop_in_place' in n,
            'carrier_tokio':lambda n:n.startswith('tokio::'),
            'carrier_timeout':lambda n:'tokio::time::timeout::' in n or 'tokio::time::timeout(' in n}
        sums={key:total(rows) for key,rows in tables.items()}
        assert sums['w2']==frozen['totals']['w2'] and sums['w3']==frozen['totals']['w3']
        groups={key:{side:total([r for r in rows if select(r['name'])]) for side,rows in tables.items()} for key,select in families.items()}
        body_rows={key:[r for r in native if select(r['name'])] for key,select in heavy.items()}
        body_single={key:bool(rows) and any(r['name'].endswith('::{closure#0}') for r in rows) and all(r['instantiation_count']==1 for r in rows) for key,rows in body_rows.items()}
        shells=[r for r in native if unary(r['name']) and r['name'].endswith('::prepare_unary_inner::{closure#0}')]
        def delta(old,new):return {k:new[k]-old[k] for k in ['definitions','instantiations','estimate']}
        boundary=groups['whole_changed_common_boundary']
        comparison={'evidence_type':'same rustc mono definition/instantiation/estimate tables; not timing, bytes or RSS allocation',
            'files':files,'totals':sums,'total_delta_w4_minus_w3':delta(sums['w3'],sums['w4']),
            'total_delta_w4_minus_w2':delta(sums['w2'],sums['w4']),
            'overlapping_source_and_carrier_families':groups,
            'whole_common_boundary_delta_w4_minus_w3':delta(boundary['w3'],boundary['w4']),
            'whole_common_boundary_estimate_change_percent':100*(boundary['w4']['estimate']/boundary['w3']['estimate']-1),
            'new_heavy_body_rows':body_rows,'new_heavy_body_each_one':body_single,
            'all_five_heavy_families_single':all(body_single.values()),'typed_prepare_body_shell_rows':shells,
            'caveats':frozen['caveats']+['New family views overlap and cannot be summed. Carrier families have no unique owner/call-edge attribution. Source-bound whole family is compared because closure names can move.','No numeric threshold is invented for meaningful collapse; report the whole-boundary measurements for Root judgment.']}
        save(out/'mono-comparison.json',comparison)
    samples=next(iter(evidence.glob('*seed-normal-lib-prebuild.native-samples.jsonl')),None)
    process={'root_rustc_peak_rss':0,'root_rustc_peak_vm_hwm':0,'root_rustc_max_threads':0,'sampled_tree_peak_rss':0,'thread_snapshots':0,'thread_names':Counter(),'max_observed_thread_cpu_percent':{}}
    first_cpu=last_cpu=None
    if samples:
        for line in samples.open():
            row=json.loads(line);first_cpu=first_cpu or row.get('cpu_stat');last_cpu=row.get('cpu_stat')
            process['sampled_tree_peak_rss']=max(process['sampled_tree_peak_rss'],row['tree_rss_sample'])
            for child in row['processes']:
                if child.get('crate_name')=='api_server' and child['comm']=='rustc':
                    process['root_rustc_peak_rss']=max(process['root_rustc_peak_rss'],child['rss'])
                    process['root_rustc_peak_vm_hwm']=max(process['root_rustc_peak_vm_hwm'],child['vm_hwm'])
                    process['root_rustc_max_threads']=max(process['root_rustc_max_threads'],child['thread_count'])
            for snapshot in row.get('rustc_thread_samples',[]):
                if snapshot.get('crate')=='api_server':
                    process['thread_snapshots']+=1
                    for thread in snapshot['threads']:
                        name=thread['name'];process['thread_names'][name]+=1
                        value=thread.get('cpu_percent_since_previous')
                        if value is not None:process['max_observed_thread_cpu_percent'][name]=max(process['max_observed_thread_cpu_percent'].get(name,0),value)
        process['parent_cpu_usage_delta_seconds']=(last_cpu['usage_usec']-first_cpu['usage_usec'])/1e6 if first_cpu and last_cpu else None
    events=dict(line.split() for line in budget.get('memory.events','').splitlines())
    final_events=(evidence/'final-cgroup.events').read_text() if (evidence/'final-cgroup.events').exists() else ''
    def ts(value):return datetime.fromisoformat(value.replace('Z','+00:00'))
    job=run_record['jobs'][0]
    complete=bool(state.get('normal_lib_complete'))
    api_valid=complete and len(roots)==1 and roots[0]['profile']['test'] is False and roots[0]['profile']['debuginfo']==0 and roots[0]['profile']['opt_level']=='0'
    summary={'status':'PASS_ORDINARY_API_LIB_SCOPE' if api_valid else 'FAIL_ORDINARY_API_LIB_SCOPE',
        'source_sha':CRATE_CANDIDATE,'run_id':receipt['run_id'],'control_sha':receipt['control_sha'],
        'CI_conclusion':receipt['CI_conclusion'],'normal_lib_complete':complete,'successful_api_root_artifact':roots,
        'actual_test_counts':None,'tail_deferred':['private24','native7','seed','MCP','application','all-targets','C01'],
        'root_build_phase':build,'failure':receipt['failure'],'fresh_completed':fresh.get('fresh_artifacts'),
        'rebuilt_completed':fresh.get('rebuilt_artifacts'),'root_diagnostic_execs':root_stage.get('diagnostic_root_execs'),
        'compiler_stages':root_stage,'mono_generation':generation,'mono_comparison_available':comparison is not None,
        'all_five_heavy_families_single':comparison['all_five_heavy_families_single'] if comparison else None,
        'whole_common_boundary_estimate_change_percent':comparison['whole_common_boundary_estimate_change_percent'] if comparison else None,
        'process_and_thread_samples':process,'parent_memory_peak_bytes':int(budget['memory.peak']) if 'memory.peak' in budget else None,
        'parent_memory_events':events,'all_command_cleanup_complete':all(row['cleanup_complete'] for row in phases),
        'final_cgroup_populated_zero':'populated 0' in final_events,'job_elapsed_seconds':(ts(job['completedAt'])-ts(job['startedAt'])).total_seconds(),
        'first_raw_attribution_delay_seconds':(ts(receipt['small_results_ready_at'])-ts(job['completedAt'])).total_seconds(),
        'raw_receipt_ready_at':receipt['small_results_ready_at'],'local_light_download_seconds':receipt['light_download']['elapsed_seconds'],
        'light_artifact':receipt['light_artifact'],'cache_artifacts_not_downloaded':receipt['cache_artifacts_not_downloaded'],
        'api_cost_summary':loaded('api-cost-summary.json'),'seed_retention':loaded('seed-retention.json'),
        'not_behavior_or_API_alltargets_or_Root_or_PR_performance_PASS':True,
        'stop_no_extension_or_same_configuration_retry':True}
    save(out/'api-result-summary.json',summary)
    lines=[summary['status'], '', 'Run '+str(receipt['run_id'])+': '+run_record['url'],
        'Source '+CRATE_CANDIDATE+'; control '+receipt['control_sha']+'.',
        'Ordinary build completed: '+str(complete)+'. Actual behavior test counts: null. Private24/native7/seed/MCP/application/all-targets/C01 deferred.',
        'API build raw phase: '+json.dumps(build),
        'Completed Fresh/rebuilt artifacts: '+str(summary['fresh_completed'])+'/'+str(summary['rebuilt_completed'])+'.',
        'Parent peak '+str(summary['parent_memory_peak_bytes'])+' bytes; memory events '+json.dumps(events)+'.',
        'Process/thread sampling: '+json.dumps(process)+'.',
        'All command cleanup complete '+str(summary['all_command_cleanup_complete'])+'; final populated0 '+str(summary['final_cgroup_populated_zero'])+'.',
        'Job '+str(summary['job_elapsed_seconds'])+'s; small direct download '+str(summary['local_light_download_seconds'])+'s; first automatic raw receipt delay '+str(summary['first_raw_attribution_delay_seconds'])+'s. Cache not downloaded.',
        '', 'Mono common-boundary evidence:']
    if comparison:
        lines+=['All five heavy-body families each one instance: '+str(comparison['all_five_heavy_families_single'])+'. Typed shell rows: '+json.dumps(comparison['typed_prepare_body_shell_rows']),
            'Whole changed common boundary estimate change vs W3 '+str(comparison['whole_common_boundary_estimate_change_percent'])+'%. This estimate is not bytes, RSS or time.',
            '', '| Table/family | W2 definitions / instances / estimate | W3 | W4 |','| --- | --- | --- | --- |']
        values={'TOTAL':comparison['totals'],**comparison['overlapping_source_and_carrier_families']}
        for family,group in values.items():
            lines.append('| '+family+' | '+' | '.join(' / '.join(str(group[side][k]) for k in ['definitions','instantiations','estimate']) for side in ['w2','w3','w4'])+' |')
    else:lines.append('No newly generated mono output available; no collapse claim.')
    lines+=['', 'Compiler pass RSS rows are completed-pass endpoints, not peaks or proof of the exact killed pass. Family views overlap and carriers are not exclusive owner allocations.',
        'A failed/unfinished API or an unconvincing whole boundary ends this path. No extended deadline, same configuration retry, test-tail continuation or per-hotspot CI was performed.',
        'Raw evidence: attribution-receipt.json, results/ci-evidence/*, mono-integrity.json, mono-comparison.json, api-result-summary.json.']
    (out/'report.md').write_text('\n'.join(lines)+'\n')
    manifest=[{'path':str(path.relative_to(out)),'bytes':path.stat().st_size,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
        for path in sorted((out/'results').rglob('*')) if path.is_file()]
    save(out/'integrity-index.json',{'run_id':receipt['run_id'],'files':manifest})
    print(json.dumps({'api_attribution':{k:summary[k] for k in ['status','run_id','normal_lib_complete','actual_test_counts','fresh_completed','rebuilt_completed','all_five_heavy_families_single','whole_common_boundary_estimate_change_percent','parent_memory_peak_bytes','first_raw_attribution_delay_seconds']},'report':str(out/'report.md')}),flush=True)

def main():
    # Authorized minimal packet: normal API lib only; no automatic test-tail/API expansion.
    candidate=os.environ['CANDIDATE_SHA']
    assert candidate==SEED_SHA==CRATE_CANDIDATE
    EVIDENCE.mkdir(parents=True,exist_ok=False);TEMP.mkdir(parents=True,exist_ok=False)
    source,target=TEMP/'seed-source',TEMP/'target-seed'
    metadata={'source_sha':candidate,'scope':'W4 ordinary API lib qualification only',
        'normal_lib_complete':False,'resumed_from_run':PRIOR_RUN,
        'tail_deferred_to_Root':True,'cache_start':'restored qualified W4 ca195 source/target; not cold'}
    initial=budget_snapshot();save(EVIDENCE/'budget-before.json',initial)
    assert int(initial['memory.max'])==8_000_000_000 and initial['memory.high']=='max' and initial['memory.swap.max']=='0'
    q,p=map(int,initial['cpu.max'].split());assert q/p<=os.cpu_count()*.6+.001
    assert TEMP==Path('/home/runner/work/_temp/issue2818')
    assert capture(['rustc','--version']).startswith('rustc 1.99.1 ')
    assert not any(os.environ.get(k) for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS'])
    save(EVIDENCE/'environment.json',{'source_sha':candidate,'rustc':capture(['rustc','-vV']),
        'cargo':capture(['cargo','-vV']),'phase':metadata['scope'],'host_cpu_count':os.cpu_count(),
        'diagnostics':'same four root-only flags as 425907; dependency rustc transparently execs',
        'not_cold_or_PR_AB':True})
    try:
        archive,incoming=download_retained()
        run('checkout-qualified-source',['git','worktree','add','--detach',str(source),candidate],seconds=120)
        run('restore-retained-source-target',['tar','-I','zstd -T1','-xf',str(archive),'--no-same-owner','-C',str(TEMP)],seconds=180)
        assert capture(['git','rev-parse','HEAD'],source)==candidate
        capture(['git','diff','--exit-code','HEAD','--'],source)
        assert target.is_dir() and any(target.rglob('*.rlib')), 'never fall back to empty target'
        assert hashlib.sha256((source/BASELINE_MOUNT_FILE).read_bytes()).hexdigest()==CRATE_FILE_SHA256[BASELINE_MOUNT_FILE]
        source_hashes={path:hashlib.sha256((source/path).read_bytes()).hexdigest() for path in API_SOURCE_SHA256}
        assert source_hashes==API_SOURCE_SHA256
        os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=candidate
        save(EVIDENCE/'source-identity.json',{'actual_sha':candidate,'clean_tracked_source':True,
            'finalization_sha256':CRATE_FILE_SHA256[BASELINE_MOUNT_FILE],'prior_crate_qualification_run':PRIOR_RUN,
            'approved_W4_source_sha256':source_hashes,'focused_tests_prequalified_not_rerun':True})
        stale=source/'api/api_server.mono_items.json'
        assert stale.is_file() and hashlib.sha256(stale.read_bytes()).hexdigest()==OLD_API_MONO_SHA256, 'fixed W3 mono provenance mismatch; no fallback'
        if stale.exists():
            assert not capture(['git','ls-files','--','api/api_server.mono_items.json'],source)
            save(EVIDENCE/'removed-stale-mono.json',{'path':str(stale),'bytes':stale.stat().st_size,
                'sha256':hashlib.sha256(stale.read_bytes()).hexdigest(),'reason':'restored old W3 statistics cannot be relabelled as new W4 candidate output'})
            stale.unlink()
        run('registry-prefetch',['cargo','fetch','--locked','--manifest-path','api/Cargo.toml'],cwd=source,seconds=900)
        env={**os.environ,'CARGO_TARGET_DIR':str(target),'CARGO_BUILD_JOBS':'1','CARGO_INCREMENTAL':'0',
             'CARGO_NET_OFFLINE':'true','CARGO_TERM_COLOR':'never','TERM':'dumb'}
        diagnostic=install_stage_wrapper(source,env)
        assert diagnostic.get('ISSUE2818_MONO_STATS')=='1', 'frozen comparison requires all four supported root diagnostics'
        save(EVIDENCE/'effective-cargo-env.json',{k:diagnostic.get(k) for k in [
            'CARGO_TARGET_DIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_NET_OFFLINE',
            'RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS',
            'ISSUE2818_RUSTC_MODE','ISSUE2818_MONO_STATS']})
        build=['cargo','build','--profile','test','--package','api-server','--features','test-support','--lib','--jobs','1','--message-format=json']
        metadata['api_build_started_wall_ns']=time.time_ns()
        metadata['actual_test_counts']=None
        metadata['deferred']=['private24','native7','seed','MCP','application','all-targets','C01']
        run('seed-normal-lib-prebuild',build,cwd=source/'api',env=diagnostic,heavy=True,seconds=600)
        metadata['normal_lib_complete']=True
        assert stage_summary()['diagnostic_root_execs']==1
        save(EVIDENCE/'status.json',{'status':'W4_NORMAL_API_LIB_COMPLETED','source_sha':candidate,
            'tail_not_run':True,'actual_test_counts':None,'remaining':'private24/native7/seed/MCP/application/alltargets/C01 explicitly deferred',
            'not_full_Root_PASS':True})
        return 0
    finally:
        cargo_fresh_summary();stage_summary();collect_mono_files(source,target)
        mono=source/'api/api_server.mono_items.json'
        proof={'source_sha':candidate,'path':str(mono),'old_mono_sha256':OLD_API_MONO_SHA256,'generation':'missing','not_RSS_or_timing':True}
        if mono.is_file():
            proof.update(bytes=mono.stat().st_size,mtime_ns=mono.stat().st_mtime_ns,
                sha256=hashlib.sha256(mono.read_bytes()).hexdigest(),api_build_started_wall_ns=metadata.get('api_build_started_wall_ns'))
            proof['generation']='new-source-bound-output' if proof['sha256']!=OLD_API_MONO_SHA256 and proof['mtime_ns']>=metadata.get('api_build_started_wall_ns',2**63) else 'INVALID_STALE_OUTPUT'
        save(EVIDENCE/'mono-generation-proof.json',proof)
        retain_seed_cache(source,target,metadata)
        save(EVIDENCE/'qualification-phase-state.json',metadata)
        save(EVIDENCE/'api-cost-summary.json',{'commands':RESULTS,
            'total_measured_command_seconds':sum(r['wall_seconds'] for r in RESULTS),
            'scope':'ordinary API lib qualification only; restored cache and changed workspace dependencies; no crate55/13 or cold/PR benefit claim'})
        save(EVIDENCE/'completion-summary.json',{'source_sha':candidate,'normal_lib_complete':metadata['normal_lib_complete'],
            'tail_not_run':True,'phases':RESULTS})
        print(json.dumps({'ci_completion':{'source_sha':candidate,'normal_lib_complete':metadata['normal_lib_complete'],
             'tail_not_run':True,'command_count':len(RESULTS)}}),flush=True)
if __name__=='__main__':
    if len(sys.argv)>1 and sys.argv[1] in ('--help','-h'):
        print('CI entry: fixed W4 ordinary API lib only. Local owner: follow-w4-api RUN_ID OUT_DIR EXPECTED_CONTROL_SHA. No automatic test tail.')
        sys.exit(0)
    if len(sys.argv)>1 and sys.argv[1]=='follow-w4-api':
        sys.exit(follow_w4(int(sys.argv[2]),sys.argv[3],sys.argv[4],api=True))
    if len(sys.argv)>1 and sys.argv[1]=='follow-w4':
        sys.exit(follow_w4(int(sys.argv[2]),sys.argv[3],sys.argv[4]))
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
