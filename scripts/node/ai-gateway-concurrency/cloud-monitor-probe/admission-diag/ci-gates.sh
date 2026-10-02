#!/usr/bin/env bash
set -euo pipefail
controls="${1:?controls}"
out=tmp/test-governance/gateway-admission-diag/build
mkdir -p "$out"
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
test "$(git rev-parse HEAD)" = "$(node -p "require('./'+process.argv[1]).product_source" "$controls/freeze.json")"
test -z "$(git status --porcelain --untracked-files=no)"
rustc --version --verbose > "$out/toolchain.txt"
cargo --version >> "$out/toolchain.txt"
node scripts/node/testing/verify-runtime.js cargo-jobs > "$out/cargo-jobs.txt"
export CARGO_BUILD_JOBS="$(cat "$out/cargo-jobs.txt")"
node "$controls/source-proof.js" > "$out/source-proof.json"
# Existing exact-source cfg(test) synchronization, never part of the release source.
# EXIT restoration also covers a failing test command without changing its exit code.
overlay_active=0
restore_test_fixture() {
 if test "$overlay_active" -eq 1;then
  python3 "$controls/test-fixture-overlay.py" --restore
  overlay_active=0
 fi
}
trap 'code=$?;restore_test_fixture || exit 99;exit "$code"' EXIT
overlay_active=1
python3 "$controls/test-fixture-overlay.py"
# Same target/toolchain; tests execute once per explicit OFF/ON process.
# No extra target builds in either measurement fixture, no Cargo on dot cloud.
for mode in 0 1;do
 export FLOWBASE_CLIENT_TRAJECTORY_DIAGNOSTICS="$mode"
 run_logged "control-plane-mode$mode" cargo test --manifest-path api/Cargo.toml --release --locked -p control-plane --lib 'client_trajectory::'
 run_logged "api-observer-mode$mode" cargo test --manifest-path api/Cargo.toml --release --locked -p api-server --lib 'client_observer::_tests'
done
node "$controls/check-gates.js" "$out" > "$out/gate-counts.json"
restore_test_fixture
test -z "$(git status --porcelain --untracked-files=no)"
export FLOWBASE_CLIENT_TRAJECTORY_DIAGNOSTICS=0
run_logged diagnostic-release-build cargo build --manifest-path api/Cargo.toml --release --locked -p api-server --bin api-server
cp api/target/release/api-server "$out/diagnostic-release-api-server"
node - "$controls/freeze.json" <<'NODE'
const fs=require('node:fs'),crypto=require('node:crypto'),cp=require('node:child_process'),assert=require('node:assert/strict');const freeze=require('./'+process.argv[2]),out='tmp/test-governance/gateway-admission-diag/build',git=args=>cp.execFileSync('git',args,{encoding:'utf8'}).trim();
assert.equal(git(['rev-parse','HEAD']),freeze.product_source);assert.equal(git(['rev-parse','HEAD:api']),freeze.product_api_tree);assert.equal(git(['status','--porcelain','--untracked-files=no']),'');
const receipt={status:'pass',product_source:freeze.product_source,api_tree:freeze.product_api_tree,profile:'release',features:'default',same_binary_all_modes:true,binary_sha256:crypto.createHash('sha256').update(fs.readFileSync(out+'/diagnostic-release-api-server')).digest('hex'),toolchain:freeze.rust_toolchain,allocator:'unchanged',codegen:'unchanged; no RUSTFLAGS override'};
fs.writeFileSync(out+'/binary-provenance.json',JSON.stringify(receipt,null,2));
NODE
printf '%s\n' 'Exact behavior gates passed in OFF/ON; single fresh diagnostic binary eligible for observer-self-cost ABBA'
