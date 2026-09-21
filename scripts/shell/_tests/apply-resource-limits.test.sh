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

grep -Fxq 'MemoryHigh=16G' "$systemd_dir/dev.slice"
grep -Fxq 'MemoryMax=18G' "$systemd_dir/dev.slice"
grep -Fxq 'MemorySwapMax=2G' "$systemd_dir/dev.slice"
grep -Fxq 'CPUQuota=1200%' "$systemd_dir/dev.slice"
grep -Fxq 'MemoryHigh=6G' "$systemd_dir/dev-frontend.slice"
grep -Fxq 'MemoryMax=8G' "$systemd_dir/dev-frontend.slice"
grep -Fxq 'MemoryLow=2G' "$systemd_dir/session.slice.d/50-memory-protection.conf"
grep -Fxq 'ManagedOOMMemoryPressureLimit=80%' \
  "$systemd_dir/app.slice.d/50-memory-budget.conf"
grep -Fxq 'MemoryHigh=9G' "$systemd_dir/dev-rust.slice"
grep -Fxq 'MemoryMax=11G' "$systemd_dir/dev-rust.slice"
grep -Fxq 'MemorySwapMax=1G' "$systemd_dir/dev-rust.slice"
grep -Fxq 'CPUQuota=' "$systemd_dir/dev-rust.slice"
grep -Fxq 'IOWeight=10' "$systemd_dir/dev-rust.slice"
grep -Fq 'memory_budget_cargo_jobs=2' "$bin_dir/cargo"
grep -Fq '"cargoJobs": 2' "$repo_dir/.1flowbase.verify.local.json"
grep -Fq '"cargoTestThreads": 2' "$repo_dir/.1flowbase.verify.local.json"
grep -Fq 'set-property --runtime dev-rust.slice MemoryHigh=9G MemoryMax=11G MemorySwapMax=1G CPUQuota= IOWeight=10' \
  "$systemctl_log"

env PATH="$mock_bin:$PATH" \
  RESOURCE_LIMITS_SYSTEMCTL_LOG="$systemctl_log" \
  RESOURCE_LIMITS_SYSTEMD_RUN_LOG="$systemd_run_log" \
  CARGO_BUILD_JOBS=12 \
  "$bin_dir/cargo" test -j 12
grep -Fxq 'CARGO_BUILD_JOBS=2' "$systemd_run_log"
grep -Fq -- '--slice=dev-rust.slice -- ' "$systemd_run_log"
grep -Fq -- 'test -j 2' "$systemd_run_log"

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

env "${common_env[@]}" "$script_dir/apply-resource-limits.sh" \
  "$script_dir/resource-limits.unlimited.example.conf"

test ! -e "$systemd_dir/session.slice.d/50-memory-protection.conf"
test ! -e "$systemd_dir/app.slice.d/50-memory-budget.conf"
test ! -e "$systemd_dir/dev.slice"
test ! -e "$systemd_dir/dev-rust.slice"
test ! -e "$bin_dir/cargo"
test ! -e "$bin_dir/dev-run"
test ! -e "$bin_dir/pnpm"
test ! -e "$systemd_dir/dev-frontend.slice"
test ! -e "$repo_dir/.1flowbase.verify.local.json"
grep -Fq 'set-property --runtime dev-rust.slice MemoryHigh=infinity MemoryMax=infinity MemorySwapMax=infinity CPUQuota= IOWeight=100' \
  "$systemctl_log"

printf 'apply-resource-limits tests passed\n'
