#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
test_root=$(mktemp -d)
trap 'rm -rf -- "$test_root"' EXIT

systemd_dir="$test_root/systemd"
bin_dir="$test_root/bin"
repo_dir="$test_root/repo"
mock_bin="$test_root/mock-bin"
systemctl_log="$test_root/systemctl.log"
systemd_run_log="$test_root/systemd-run.log"
mkdir -p "$systemd_dir" "$bin_dir" "$repo_dir" "$mock_bin"

# Exercise the real Node calculation with deterministic available CPU counts.
cat >"$test_root/available-cpus.cjs" <<'EOF'
require('node:os').availableParallelism = () => Number(process.env.RESOURCE_LIMITS_TEST_CPUS);
require('node:os').totalmem = () => Number(process.env.RESOURCE_LIMITS_TEST_MEMORY_GIB) * 2 ** 30;
process.constrainedMemory = () => Number(process.env.RESOURCE_LIMITS_TEST_CONSTRAINED_GIB || 0) * 2 ** 30;
EOF
export NODE_OPTIONS="${NODE_OPTIONS:+$NODE_OPTIONS }--require=$test_root/available-cpus.cjs"
export RESOURCE_LIMITS_TEST_CPUS=20
export RESOURCE_LIMITS_TEST_MEMORY_GIB=24
export RESOURCE_LIMITS_TEST_CONSTRAINED_GIB=0

cat >"$mock_bin/systemctl" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"${RESOURCE_LIMITS_SYSTEMCTL_LOG:?}"
if [[ ${RESOURCE_LIMITS_BLOCKING_PID_FILE:-} && $* == *'--no-block stop rust-cargo-'* ]]; then
  kill "$(cat "$RESOURCE_LIMITS_BLOCKING_PID_FILE")" 2>/dev/null || true
fi
if [[ ${RESOURCE_LIMITS_MANAGER_UNAVAILABLE:-} == 1 && $* == *'is-active default.target'* ]]; then
  exit 1
fi
EOF
chmod +x "$mock_bin/systemctl"

cat >"$mock_bin/systemd-run" <<'EOF'
#!/usr/bin/env bash
printf 'CARGO_BUILD_JOBS=%s\n' "${CARGO_BUILD_JOBS:-}" >>"${RESOURCE_LIMITS_SYSTEMD_RUN_LOG:?}"
printf '%s\n' "$*" >>"${RESOURCE_LIMITS_SYSTEMD_RUN_LOG:?}"
if [[ ${RESOURCE_LIMITS_BLOCKING_PID_FILE:-} ]]; then
  printf '%s\n' "$$" >"$RESOURCE_LIMITS_BLOCKING_PID_FILE"
  exec sleep 30
fi
EOF
chmod +x "$mock_bin/systemd-run"

common_env=(
  RESOURCE_LIMITS_SYSTEMD_USER_DIR="$systemd_dir"
  RESOURCE_LIMITS_USER_BIN_DIR="$bin_dir"
  RESOURCE_LIMITS_REPO_ROOT="$repo_dir"
  RESOURCE_LIMITS_SYSTEMCTL_BIN="$mock_bin/systemctl"
  RESOURCE_LIMITS_SYSTEMCTL_LOG="$systemctl_log"
  RESOURCE_LIMITS_REAL_CARGO="$test_root/real-cargo"
)

env "${common_env[@]}" "$script_dir/apply-resource-limits.sh" \
  "$script_dir/resource-limits.conf"

grep -Fxq 'ManagedOOMSwap=auto' "$systemd_dir/dev.slice"
grep -Fxq 'ManagedOOMSwap=kill' "$systemd_dir/dev-rust.slice"
grep -Fxq 'MemoryHigh=infinity' "$systemd_dir/dev.slice"
grep -Fxq 'MemoryMax=infinity' "$systemd_dir/dev.slice"
grep -Fxq 'MemorySwapMax=infinity' "$systemd_dir/dev.slice"
grep -Fxq 'CPUQuota=1000%' "$systemd_dir/dev.slice"
grep -Fxq 'MemoryHigh=infinity' "$systemd_dir/dev-frontend.slice"
grep -Fxq 'MemoryMax=infinity' "$systemd_dir/dev-frontend.slice"
grep -Fxq 'MemoryLow=2147483648' "$systemd_dir/session.slice.d/50-memory-protection.conf"
grep -Fxq 'ManagedOOMMemoryPressure=auto' \
  "$systemd_dir/app.slice.d/50-memory-budget.conf"
rust_high=$((11059 * 1024 * 1024))
rust_max=$((12 * 1024 * 1024 * 1024))
grep -Fxq "MemoryHigh=$rust_high" "$systemd_dir/dev-rust.slice"
grep -Fxq "MemoryMax=$rust_max" "$systemd_dir/dev-rust.slice"
grep -Fxq 'MemorySwapMax=1G' "$systemd_dir/dev-rust.slice"
grep -Fxq 'CPUQuota=' "$systemd_dir/dev-rust.slice"
grep -Fxq 'IOWeight=10' "$systemd_dir/dev-rust.slice"
grep -Fq 'default_cargo_job_limit=auto' "$bin_dir/cargo"
grep -Fq '"cargoJobs": "auto"' "$repo_dir/.1flowbase.verify.local.json"
grep -Fq '"cargoTestThreads": "auto"' "$repo_dir/.1flowbase.verify.local.json"
test -x "$bin_dir/oneflowbase-resource-budget"
grep -Fq "set-property --runtime dev-rust.slice MemoryHigh=$rust_high MemoryMax=$rust_max MemorySwapMax=1G CPUQuota= IOWeight=10" \
  "$systemctl_log"

# Ordinary Cargo/pnpm commands must not enter the optional heavy-job queue.
cp "$bin_dir/dev-heavy-run" "$test_root/installed-heavy-run"
cat >"$bin_dir/dev-heavy-run" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >>"${RESOURCE_LIMITS_SYSTEMD_RUN_LOG:?}.gate"
exec "$@"
EOF
chmod +x "$bin_dir/dev-heavy-run"

env PATH="$mock_bin:$PATH" \
  RESOURCE_LIMITS_SYSTEMCTL_LOG="$systemctl_log" \
  RESOURCE_LIMITS_SYSTEMD_RUN_LOG="$systemd_run_log" \
  CARGO_BUILD_JOBS=12 \
  "$bin_dir/cargo" test -j 12
grep -Fxq 'CARGO_BUILD_JOBS=10' "$systemd_run_log"
grep -Fq -- '--slice=dev-rust.slice -- ' "$systemd_run_log"
grep -Fq -- 'test -j 10' "$systemd_run_log"
test ! -e "$systemd_run_log.gate"

# An invocation can request a smaller budget without changing the memory scope.
env PATH="$mock_bin:$PATH" \
  RESOURCE_LIMITS_SYSTEMCTL_LOG="$systemctl_log" \
  RESOURCE_LIMITS_SYSTEMD_RUN_LOG="$test_root/warmup-run.log" \
  ONEFLOWBASE_CARGO_JOB_LIMIT=4 CARGO_BUILD_JOBS=10 \
  "$bin_dir/cargo" test --jobs=10
grep -Fxq 'CARGO_BUILD_JOBS=4' "$test_root/warmup-run.log"
grep -Fq -- '--slice=dev-rust.slice -- ' "$test_root/warmup-run.log"
grep -Fq -- 'test --jobs=4' "$test_root/warmup-run.log"
grep -Fq -- '--property=CPUQuota=400%' "$test_root/warmup-run.log"

# The override is per invocation; ordinary commands retain the default cap.
env PATH="$mock_bin:$PATH" \
  RESOURCE_LIMITS_SYSTEMCTL_LOG="$systemctl_log" \
  RESOURCE_LIMITS_SYSTEMD_RUN_LOG="$test_root/default-run.log" \
  CARGO_BUILD_JOBS=12 "$bin_dir/cargo" build -j12
grep -Fxq 'CARGO_BUILD_JOBS=10' "$test_root/default-run.log"
grep -Fq -- 'build -j10' "$test_root/default-run.log"
! grep -Fq -- '--property=CPUQuota=' "$test_root/default-run.log"

set +e
ONEFLOWBASE_CARGO_JOB_LIMIT=invalid "$bin_dir/cargo" build >"$test_root/invalid-budget.log" 2>&1
budget_status=$?
set -e
test "$budget_status" -eq 2
grep -Fq 'ONEFLOWBASE_CARGO_JOB_LIMIT must be a positive integer' "$test_root/invalid-budget.log"

# pnpm must enter the frontend child of the common development budget.
cat >"$mock_bin/pnpm" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod +x "$mock_bin/pnpm"
env PATH="$bin_dir:$mock_bin:$PATH" \
  RESOURCE_LIMITS_SYSTEMCTL_LOG="$systemctl_log" \
  RESOURCE_LIMITS_SYSTEMD_RUN_LOG="$systemd_run_log" \
  "$bin_dir/pnpm" --version
grep -Fq -- "--slice=dev-frontend.slice -- $mock_bin/pnpm --version" "$systemd_run_log"

env PATH="$bin_dir:$mock_bin:$PATH" \
  RESOURCE_LIMITS_SYSTEMCTL_LOG="$systemctl_log" \
  RESOURCE_LIMITS_SYSTEMD_RUN_LOG="$systemd_run_log" \
  "$bin_dir/pnpm" -w build
test ! -e "$systemd_run_log.gate"
env PATH="$bin_dir:$mock_bin:$PATH" \
  RESOURCE_LIMITS_SYSTEMCTL_LOG="$systemctl_log" \
  RESOURCE_LIMITS_SYSTEMD_RUN_LOG="$systemd_run_log" \
  "$bin_dir/pnpm" test --watch
test ! -e "$systemd_run_log.gate"

# A missing manager must not silently start an unrestricted build.
set +e
env PATH="$mock_bin:$PATH" RESOURCE_LIMITS_SYSTEMCTL_LOG="$systemctl_log" \
  RESOURCE_LIMITS_MANAGER_UNAVAILABLE=1 "$bin_dir/cargo" build >"$test_root/unavailable.log" 2>&1
unavailable_status=$?
set -e
test "$unavailable_status" -eq 2
grep -Fq 'resource limits unavailable' "$test_root/unavailable.log"

# A cancelled wrapper requests shutdown of its own scope and returns promptly.
blocking_pid_file="$test_root/blocking.pid"
env PATH="$mock_bin:$PATH" RESOURCE_LIMITS_SYSTEMCTL_LOG="$systemctl_log" \
  RESOURCE_LIMITS_SYSTEMD_RUN_LOG="$systemd_run_log" \
  RESOURCE_LIMITS_BLOCKING_PID_FILE="$blocking_pid_file" "$bin_dir/cargo" build &
wrapper_pid=$!
for ((attempt=0; attempt<40; attempt++)); do
  [[ -s $blocking_pid_file ]] && break
  sleep 0.05
done
test -s "$blocking_pid_file"
kill -TERM "$wrapper_pid"
set +e
wait "$wrapper_pid"
cancel_status=$?
set -e
test "$cancel_status" -eq 143
grep -Fq -- '--no-block stop rust-cargo-' "$systemctl_log"

# Installation resolves systemd quotas; wrappers recalculate for every call.
for cpu_case in 1:1 2:2 3:2 4:2 8:4 20:10; do
  cpus=${cpu_case%:*}
  jobs=${cpu_case#*:}
  env "${common_env[@]}" RESOURCE_LIMITS_TEST_CPUS="$cpus" \
    "$script_dir/apply-resource-limits.sh" "$script_dir/resource-limits.conf"
  grep -Fxq "CPUQuota=${jobs}00%" "$systemd_dir/dev.slice"
  grep -Fq '"cargoJobs": "auto"' "$repo_dir/.1flowbase.verify.local.json"
  env PATH="$mock_bin:$PATH" RESOURCE_LIMITS_TEST_CPUS="$cpus" \
    RESOURCE_LIMITS_SYSTEMCTL_LOG="$systemctl_log" \
    RESOURCE_LIMITS_SYSTEMD_RUN_LOG="$test_root/cpu-$cpus.log" \
    CARGO_BUILD_JOBS=40 "$bin_dir/cargo" build --jobs=40
  grep -Fxq "CARGO_BUILD_JOBS=$jobs" "$test_root/cpu-$cpus.log"
  grep -Fq -- "build --jobs=$jobs" "$test_root/cpu-$cpus.log"
done

# Rust's memory floor may exceed half the machine but never consumes OS headroom.
for memory_case in 4:1:3 8:2:6 12:2:8 16:2:8 24:2:12 32:2:16; do
  IFS=: read -r capacity reserve budget <<<"$memory_case"
  env "${common_env[@]}" RESOURCE_LIMITS_TEST_MEMORY_GIB="$capacity" \
    "$script_dir/apply-resource-limits.sh" "$script_dir/resource-limits.conf"
  grep -Fxq "MemoryMax=$((budget * 1024 * 1024 * 1024))" "$systemd_dir/dev-rust.slice"
  grep -Fxq "MemoryLow=$((reserve * 1024 * 1024 * 1024))" "$systemd_dir/session.slice.d/50-memory-protection.conf"
done
env "${common_env[@]}" RESOURCE_LIMITS_TEST_MEMORY_GIB=64 RESOURCE_LIMITS_TEST_CONSTRAINED_GIB=8 \
  "$script_dir/apply-resource-limits.sh" "$script_dir/resource-limits.conf"
grep -Fxq 'MemoryMax=6442450944' "$systemd_dir/dev-rust.slice"

# The same installed wrapper follows changed CPU availability without reinstall.
env PATH="$mock_bin:$PATH" RESOURCE_LIMITS_TEST_CPUS=8 \
  RESOURCE_LIMITS_SYSTEMCTL_LOG="$systemctl_log" \
  RESOURCE_LIMITS_SYSTEMD_RUN_LOG="$test_root/migrated.log" \
  "$bin_dir/cargo" build
grep -Fxq 'CARGO_BUILD_JOBS=4' "$test_root/migrated.log"

# Explicit lower job counts remain effective with automatic defaults.
env PATH="$mock_bin:$PATH" \
  RESOURCE_LIMITS_SYSTEMCTL_LOG="$systemctl_log" \
  RESOURCE_LIMITS_SYSTEMD_RUN_LOG="$test_root/lower.log" \
  CARGO_BUILD_JOBS=2 "$bin_dir/cargo" test -j2
grep -Fxq 'CARGO_BUILD_JOBS=2' "$test_root/lower.log"
grep -Fq -- 'test -j2' "$test_root/lower.log"

# Custom fixed profiles remain supported and emit numeric JSON values.
cat >"$test_root/fixed.conf" <<EOF
source '$script_dir/resource-limits.conf'
DEV_CPU_QUOTA=300%
CARGO_MAX_JOBS=3
PROJECT_CARGO_JOBS=2
PROJECT_CARGO_TEST_THREADS=1
SESSION_MEMORY_LOW=512M
RUST_MEMORY_HIGH=2G
RUST_MEMORY_MAX=3G
EOF
env "${common_env[@]}" "$script_dir/apply-resource-limits.sh" "$test_root/fixed.conf"
grep -Fxq 'CPUQuota=300%' "$systemd_dir/dev.slice"
grep -Fq 'default_cargo_job_limit=3' "$bin_dir/cargo"
grep -Fq '"cargoJobs": 2' "$repo_dir/.1flowbase.verify.local.json"
grep -Fq '"cargoTestThreads": 1' "$repo_dir/.1flowbase.verify.local.json"
grep -Fxq 'MemoryHigh=2G' "$systemd_dir/dev-rust.slice"
grep -Fxq 'MemoryMax=3G' "$systemd_dir/dev-rust.slice"
grep -Fxq 'MemoryLow=512M' "$systemd_dir/session.slice.d/50-memory-protection.conf"

env "${common_env[@]}" "$script_dir/apply-resource-limits.sh" \
  "$script_dir/resource-limits.unlimited.example.conf"

test ! -e "$systemd_dir/session.slice.d/50-memory-protection.conf"
test ! -e "$systemd_dir/app.slice.d/50-memory-budget.conf"
test ! -e "$systemd_dir/dev.slice"
test ! -e "$systemd_dir/dev-rust.slice"
test ! -e "$bin_dir/cargo"
test ! -e "$bin_dir/dev-run"
test ! -e "$bin_dir/pnpm"
test ! -e "$bin_dir/dev-heavy-run"
test ! -e "$bin_dir/oneflowbase-resource-budget"
test ! -e "$systemd_dir/dev-frontend.slice"
test ! -e "$repo_dir/.1flowbase.verify.local.json"
grep -Fq 'set-property --runtime dev-rust.slice MemoryHigh=infinity MemoryMax=infinity MemorySwapMax=infinity CPUQuota= IOWeight=100' \
  "$systemctl_log"

printf 'apply-resource-limits tests passed\n'
