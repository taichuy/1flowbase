#!/usr/bin/env bash
set -euo pipefail
controls="${1:?controls}"
root=tmp/test-governance/gateway-identity-reuse
out="$root/build"
mkdir -p "$out"
cp "$controls/freeze.json" "$out/freeze.json"
printf '%s\n' "$WORKFLOW_SHA" > "$out/harness-sha.txt"
run_logged() {
 local name="$1";shift
 printf '[phase-start] %s %s\n' "$name" "$(date -u +%FT%TZ)"
 set +e
 "$@" > "$out/$name.log" 2>&1
 local code=$?
 set -e
 printf '[phase-end] %s %s exit=%s\n' "$name" "$(date -u +%FT%TZ)" "$code"
 if test "$code" -ne 0;then tail -c 6000 "$out/$name.log";return "$code";fi
}
rustc --version --verbose > "$out/toolchain.txt"
cargo --version >> "$out/toolchain.txt"
node scripts/node/testing/verify-runtime.js cargo-jobs > "$out/cargo-jobs.txt"
export CARGO_BUILD_JOBS="$(cat "$out/cargo-jobs.txt")"
node "$controls/source-proof.js" > "$out/source-structure.json"
printf '%s\n' 'ordinary identity; no sudo/perf/maps/credential/security changes; protected PG PSS/IO null' > "$out/sampling-boundary.txt"
# Six pure goldens and existing real PostgreSQL positive/negative cases; one Cargo at a time.
for filter in canonical_runtime_json client_semantic client_dense_directory runtime_json_storage_tests 'sequencing::tests';do
 name="pg-${filter//::/-}"
 run_logged "$name" cargo test --manifest-path api/Cargo.toml --release --locked -p storage-durable-postgres --lib "$filter"
done
python3 - "$out" <<'PYGATE'
import json,pathlib,re,sys
out=pathlib.Path(sys.argv[1]);counts=[]
required={
 'pg-canonical_runtime_json.log':['prepared_identity_matches_independent_nested_order_and_unicode_golden','prepared_identity_preserves_numeric_value_kinds_and_extreme_integers','prepared_identity_distinguishes_original_nul_and_literal_escape','prepared_identity_keeps_array_order_and_one_borrowed_value','prepared_identity_handles_empty_values_without_body_or_history_cache','prepared_identity_can_move_digest_without_another_hash_or_value_clone'],
 'pg-client_semantic.log':['client_semantic_directories_share_only_complete_original_values_and_preserve_occurrences','client_semantic_locator_hash_and_content_scope_negatives_reject_wrong_values','client_semantic_history_preserves_anchors_originals_cursors_and_retained_nul_progress','client_semantic_high_water_preserves_batch_and_concurrent_runtime_sequences'],
 'pg-client_dense_directory.log':['client_dense_directory_nul_layout_zero_preserves_unparsed_original_numeric_tokens'],
 'pg-runtime_json_storage_tests.log':[],
 'pg-sequencing-tests.log':['rolled_back_and_failed_appends_do_not_commit_a_reservation','sequence_overflow_is_an_error_and_preserves_committed_rows','concurrent_batches_and_direct_reservations_have_disjoint_monotonic_ranges']}
for name,cases in required.items():
 text=(out/name).read_text();result=re.findall(r'test result: ok[.] (\d+) passed; (\d+) failed',text);assert result and sum(int(p) for p,f in result)>0,name
 for case in cases:assert re.search(r'test [^\n]*'+re.escape(case)+r' [.]{3} ok',text),'missing passing case '+case
 counts.append({'log':name,'passed':sum(int(p) for p,f in result),'failed':sum(int(f) for p,f in result),'required':cases})
(out/'gate-counts.json').write_text(json.dumps(counts,indent=2))
PYGATE
# Exact baseline and candidate release binaries are built on Actions only.
for arm in baseline candidate;do
 source="$(node -p "require('./'+process.argv[1])[process.argv[2]]" "$controls/freeze.json" "$arm")"
 git checkout --detach "$source"
 test -z "$(git status --porcelain --untracked-files=no)"
 run_logged "$arm-release-build" cargo build --manifest-path api/Cargo.toml --release --locked -p api-server --bin api-server
 cp api/target/release/api-server "$out/$arm-release-api-server"
 node - "$arm" "$source" <<'NODE'
const fs=require('node:fs'),crypto=require('node:crypto'),cp=require('node:child_process');
const [arm,source]=process.argv.slice(2),out='tmp/test-governance/gateway-identity-reuse/build',file=out+'/binaries.json',all=fs.existsSync(file)?JSON.parse(fs.readFileSync(file)):{};
all[arm]={source,api_tree:cp.execFileSync('git',['rev-parse','HEAD:api'],{encoding:'utf8'}).trim(),profile:'release',features:'default',sha256:crypto.createHash('sha256').update(fs.readFileSync(out+'/'+arm+'-release-api-server')).digest('hex')};
fs.writeFileSync(file,JSON.stringify(all,null,2));
NODE
done
node - "$controls/freeze.json" <<'NODE'
const fs=require('node:fs'),assert=require('node:assert/strict');const freeze=JSON.parse(fs.readFileSync(process.argv[2])),out='tmp/test-governance/gateway-identity-reuse/build',b=JSON.parse(fs.readFileSync(out+'/binaries.json'));
for(const arm of ['baseline','candidate']){assert.equal(b[arm].source,freeze[arm]);assert.equal(b[arm].api_tree,freeze[arm+'_api_tree']);}
fs.writeFileSync(out+'/source-proof.json',JSON.stringify({baseline:freeze.baseline,candidate:freeze.candidate,baseline_binary_sha256:b.baseline.sha256,scope:'single identity-reuse source delta; precise source/API tree/binary provenance'},null,2));
NODE
printf '%s\n' 'Relevant Rust gates and exact paired release builds passed; synthetic resource stage eligible'
