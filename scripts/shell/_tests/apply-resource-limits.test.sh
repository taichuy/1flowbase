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
EOF
chmod +x "$mock_bin/systemctl"

cat >"$mock_bin/systemd-run" <<'EOF'
#!/usr/bin/env bash
printf 'CARGO_BUILD_JOBS=%s\n' "${CARGO_BUILD_JOBS:-}" >>"${RESOURCE_LIMITS_SYSTEMD_RUN_LOG:?}"
printf '%s\n' "$*" >>"${RESOURCE_LIMITS_SYSTEMD_RUN_LOG:?}"
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

grep -Fxq 'MemoryLow=2G' "$systemd_dir/session.slice.d/50-memory-protection.conf"
grep -Fxq 'ManagedOOMMemoryPressureLimit=40%' \
  "$systemd_dir/app.slice.d/50-memory-budget.conf"
grep -Fxq 'MemoryHigh=6G' "$systemd_dir/rust-build.slice"
grep -Fxq 'MemoryMax=8G' "$systemd_dir/rust-build.slice"
grep -Fxq 'MemorySwapMax=3G' "$systemd_dir/rust-build.slice"
grep -Fq 'memory_budget_cargo_jobs=6' "$bin_dir/cargo"
grep -Fq '"cargoJobs": 2' "$repo_dir/.1flowbase.verify.local.json"
grep -Fq '"cargoTestThreads": 4' "$repo_dir/.1flowbase.verify.local.json"
grep -Fq 'set-property --runtime rust-build.slice MemoryHigh=6G MemoryMax=8G MemorySwapMax=3G' \
  "$systemctl_log"

env PATH="$mock_bin:$PATH" \
  RESOURCE_LIMITS_SYSTEMCTL_LOG="$systemctl_log" \
  RESOURCE_LIMITS_SYSTEMD_RUN_LOG="$systemd_run_log" \
  CARGO_BUILD_JOBS=12 \
  "$bin_dir/cargo" test -j 12
grep -Fxq 'CARGO_BUILD_JOBS=6' "$systemd_run_log"
grep -Fq -- '--slice=rust-build.slice -- ' "$systemd_run_log"
grep -Fq -- 'test -j 6' "$systemd_run_log"

env "${common_env[@]}" "$script_dir/apply-resource-limits.sh" \
  "$script_dir/resource-limits.unlimited.example.conf"

test ! -e "$systemd_dir/session.slice.d/50-memory-protection.conf"
test ! -e "$systemd_dir/app.slice.d/50-memory-budget.conf"
test ! -e "$systemd_dir/dev.slice"
test ! -e "$systemd_dir/rust-build.slice"
test ! -e "$bin_dir/cargo"
test ! -e "$repo_dir/.1flowbase.verify.local.json"
grep -Fq 'set-property --runtime rust-build.slice MemoryHigh=infinity MemoryMax=infinity MemorySwapMax=infinity' \
  "$systemctl_log"

printf 'apply-resource-limits tests passed\n'
