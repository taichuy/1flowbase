#!/usr/bin/env bash
set -euo pipefail
controls="${1:?controls directory}"
evidence=tmp/test-governance/gateway-resource-dev/build
mkdir -p "$evidence"
printf '%s\n' "$WORKFLOW_SHA" > "$evidence/harness-sha.txt"
cp "$controls/freeze.json" "$evidence/freeze.json"
printf '%s\n' '{"resources_dispatched":false,"reason":"Differential section/commit/cancel and UI/replay/integrity/aftercommit contract gates pending main."}' > "$evidence/resource-block.json"
run_logged() {
 local phase="$1"; local logfile="$2"; shift 2
 printf '[phase-start] %s %s\n' "$phase" "$(date -u +%FT%TZ)"
 set +e
 "$@" 2>&1 | tee "$logfile"
 local statuses=("${PIPESTATUS[@]}")
 set -e
 printf '[phase-end] %s %s command_exit=%s tee_exit=%s\n' "$phase" "$(date -u +%FT%TZ)" "${statuses[0]}" "${statuses[1]}"
 test "${statuses[0]}" -eq 0 && test "${statuses[1]}" -eq 0
}
node "$controls/source-proof.js" --source-only
rustc --version --verbose > "$evidence/toolchain.txt"
cargo --version >> "$evidence/toolchain.txt"
uname -a > "$evidence/runner-kernel.txt"
lscpu > "$evidence/runner-cpu.txt"
cat /proc/meminfo > "$evidence/runner-memory.txt"
node scripts/node/testing/verify-runtime.js cargo-jobs > "$evidence/cargo-jobs.txt"
export CARGO_BUILD_JOBS="$(cat "$evidence/cargo-jobs.txt")"
python3 - <<'FACTS'
import pathlib,json
p=pathlib.Path('/sys/fs/cgroup');names=['cpu.max','cpuset.cpus.effective','memory.max','memory.high','cpu.stat','io.stat'];out={}
for name in names:
 try:out[name]=(p/name).read_text().strip()
 except OSError as e:out[name]={'unavailable':e.__class__.__name__}
pathlib.Path('tmp/test-governance/gateway-resource-dev/build/runner-quota.json').write_text(json.dumps(out,indent=2))
FACTS
# Frozen contracts compilation is separate from behavioral adapter/PG evidence.
run_logged contracts "$evidence/contracts-tests.log" cargo test --manifest-path api/Cargo.toml --release --locked -p control-plane-contracts
run_logged dependency-metadata "$evidence/dependency-metadata.json" cargo metadata --manifest-path api/Cargo.toml --locked --no-deps --format-version 1
python3 - <<'BOUNDARY'
import json,pathlib
p=pathlib.Path('tmp/test-governance/gateway-resource-dev/build');meta=json.loads((p/'dependency-metadata.json').read_text());package=next(x for x in meta['packages'] if x['name']=='control-plane-contracts');deps=[x['name'] for x in package['dependencies']];forbidden={'control-plane','storage-durable-postgres','api-server','runtime-extension-host'};assert not forbidden.intersection(deps),deps
(p/'contracts-boundary.json').write_text(json.dumps({'direct_dependencies':deps,'forbidden_dependencies_absent':True,'scope':'Cargo direct dependency metadata only, not runtime ownership proof'},indent=2))
BOUNDARY
run_logged adapter-compile "$evidence/adapter-test-compile.log" cargo test --manifest-path api/Cargo.toml --release --locked -p storage-durable-postgres --lib --no-run
run_logged candidate-pg-trajectory "$evidence/pg-trajectory-tests.log" cargo test --manifest-path api/Cargo.toml --release --locked -p storage-durable-postgres --lib client_trajectory
run_logged candidate-pg-sequencing "$evidence/pg-sequencing-tests.log" cargo test --manifest-path api/Cargo.toml --release --locked -p storage-durable-postgres --lib orchestration_runtime_repository::sequencing::tests
run_logged candidate-classifier "$evidence/control-plane-trajectory-tests.log" cargo test --manifest-path api/Cargo.toml --release --locked -p control-plane --lib client_trajectory
python3 - "$controls/freeze.json" <<'GATES'
import pathlib,re,json,sys
root=pathlib.Path('tmp/test-governance/gateway-resource-dev/build');freeze=json.loads(pathlib.Path(sys.argv[1]).read_text());required={
'contracts-tests.log':['atomic_client_trajectory_port_accepts_existing_dto_and_remains_object_safe'],
'pg-trajectory-tests.log':['atomic_step_batch_rolls_back_every_row_and_reuses_range_after_late_failure','atomic_step_batch_rejects_cross_capture_and_section_owner_without_writes','atomic_step_batches_serialize_with_single_writers_and_fk_readers','client_trajectory_held_run_lock_serializes_nonraw_facts_and_allows_fk_reader','client_trajectory_held_run_lock_rolls_back_failed_section_and_reuses_sequence'],
'pg-sequencing-tests.log':['atomic_range_reservation_repairs_reserved_tail_and_rolls_back_without_gaps','rolled_back_and_failed_appends_do_not_commit_a_reservation','sequence_overflow_is_an_error_and_preserves_committed_rows','concurrent_batches_and_direct_reservations_have_disjoint_monotonic_ranges'],
'control-plane-trajectory-tests.log':['natural_step_batches_preserve_exact_sections_and_timing_order','unsupported_batch_keeps_individual_failure_accounting_and_suffix','atomic_failure_counts_all_unpersisted_facts_without_individual_retry','idle_frames_are_committed_without_terminal_and_owner_drop_flushes_accepted_suffix']};rows=[]
for name,cases in required.items():
 text=(root/name).read_text();counts=re.findall(r'test result: ok[.] (\d+) passed; (\d+) failed',text);assert counts and sum(int(x[0]) for x in counts)>0,name
 for case in cases:assert re.search(r'test [^\n]*'+re.escape(case)+r' [.]{3} ok',text),'missing passing case '+case
 rows.append({'log':name,'candidate':freeze['candidate'],'passed':sum(int(x[0]) for x in counts),'failed':sum(int(x[1]) for x in counts),'required_cases':cases})
(root/'gate-counts.json').write_text(json.dumps(rows,indent=2))
(root/'resource-block.json').write_text(json.dumps({'resources_dispatched':False,'reason':'New per-section/commit/cancel differential fault gates and UI/replay/integrity/aftercommit contract equivalence pending main owner; atomic rollback alone is insufficient.'},indent=2))
GATES
# Both updated-dev arms are compiled fresh on this runner, using the same target/toolchain/profile.
for arm in baseline candidate; do
 frozen_sha="$(node -p "const p=require('./'+process.argv[1]);p[process.argv[2]==='baseline'?'common_baseline':'candidate']" "$controls/freeze.json" "$arm")"
 git checkout --detach "$frozen_sha"
 printf '[build-source] arm=%s sha=%s profile=release features=default RUSTFLAGS=%s\n' "$arm" "$(git rev-parse HEAD)" "${RUSTFLAGS:-}"
 run_logged "$arm-release" "$evidence/$arm-build.log" cargo build --manifest-path api/Cargo.toml --release --locked -p api-server --bin api-server
 cp api/target/release/api-server "$evidence/$arm-release-api-server"
 node - "$arm" "$frozen_sha" <<'HASH'
const fs=require('node:fs'),crypto=require('node:crypto'),[arm,source]=process.argv.slice(2),out='tmp/test-governance/gateway-resource-dev/build',file=out+'/binaries.json',data=fs.existsSync(file)?JSON.parse(fs.readFileSync(file)):{};data[arm]={source,profile:'release',features:'default',sha256:crypto.createHash('sha256').update(fs.readFileSync(out+'/'+arm+'-release-api-server')).digest('hex')};fs.writeFileSync(file,JSON.stringify(data,null,2));
HASH
done
node "$controls/source-proof.js"
printf '%s\n' 'Correctness/build gates complete; resource execution blocked pending differential fault contract evidence.'
