#!/usr/bin/env bash
set -euo pipefail
# Each test declares its own context; caller context must not mask default paths.
unset ONEFLOWBASE_TASK_BUDGET_CGROUP ONEFLOWBASE_TASK_BUDGET_ID
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
source "$script_dir/resource-budget-context.sh"
test_root=$(mktemp -d)
cleanup() {
  [[ -z ${own:-} ]] || kill -TERM "$own" 2>/dev/null || true
  [[ -z ${sibling:-} ]] || kill -TERM "$sibling" 2>/dev/null || true
  rm -rf -- "$test_root"
}
trap cleanup EXIT
mkdir -p "$test_root/cgroup/task/child"
printf '1200000 100000\n' > "$test_root/cgroup/task/cpu.max"
printf '8000000000\n' > "$test_root/cgroup/task/memory.max"
identity=$(stat -c %i "$test_root/cgroup/task")
oneflowbase_budget_validate "$test_root/cgroup" /task/child /task "$identity"
for bad_path in /other /task/../task /task/child; do
  ! oneflowbase_budget_validate "$test_root/cgroup" /task/child "$bad_path" "$identity"
done
! oneflowbase_budget_validate "$test_root/cgroup" /outside /task "$identity"
! oneflowbase_budget_validate "$test_root/cgroup" /task/child /task 1
printf 'max 100000\n' > "$test_root/cgroup/task/cpu.max"
! oneflowbase_budget_validate "$test_root/cgroup" /task/child /task "$identity"
printf '1200000 100000\n' > "$test_root/cgroup/task/cpu.max"
printf 'max\n' > "$test_root/cgroup/task/memory.max"
! oneflowbase_budget_validate "$test_root/cgroup" /task/child /task "$identity"
echo 'budget validator positive/ancestry/canonical-path/identity/unlimited tests passed'

# Optional integration requires an already configured finite cgroup ancestor.
# It never installs a production profile or changes resource limits.
if [[ -z ${RESOURCE_LIMITS_TEST_TASK_CGROUP:-} ]]; then
  echo 'real systemd integration skipped (RESOURCE_LIMITS_TEST_TASK_CGROUP unset)'
  exit 0
fi
budget=$RESOURCE_LIMITS_TEST_TASK_CGROUP
identity=$(stat -c %i "/sys/fs/cgroup$budget")
oneflowbase_budget_validate /sys/fs/cgroup "$(sed -n 's/^0:://p' /proc/self/cgroup)" "$budget" "$identity"
mkdir -p "$test_root/bin" "$test_root/mock" "$test_root/repo" "$test_root/systemd"
printf '#!/bin/sh\nexit 0\n' > "$test_root/mock/systemctl"
chmod +x "$test_root/mock/systemctl"
cat > "$test_root/fixed.conf" <<EOF
source '$script_dir/resource-limits.conf'
SESSION_MEMORY_LOW=1M
RUST_MEMORY_HIGH=2G
RUST_MEMORY_MAX=3G
DEV_CPU_QUOTA=300%
RUST_CPU_QUOTA=200%
CARGO_MAX_JOBS=2
PROJECT_CARGO_JOBS=2
PROJECT_CARGO_TEST_THREADS=1
EOF
cat > "$test_root/probe" <<'EOF'
#!/bin/bash
set -euo pipefail
cat /proc/self/cgroup > "${RESOURCE_LIMITS_PROBE_LOG:?}"
if [[ ${RESOURCE_LIMITS_PROBE_SLEEP:-} == 1 ]]; then
  printf '%s\n' "$$" > "$RESOURCE_LIMITS_PROBE_LOG.pid"
  exec sleep 30
fi
EOF
chmod +x "$test_root/probe"
RESOURCE_LIMITS_SYSTEMD_USER_DIR="$test_root/systemd" RESOURCE_LIMITS_USER_BIN_DIR="$test_root/bin" RESOURCE_LIMITS_REPO_ROOT="$test_root/repo" RESOURCE_LIMITS_SYSTEMCTL_BIN="$test_root/mock/systemctl" RESOURCE_LIMITS_REAL_CARGO="$test_root/probe" bash "$script_dir/apply-resource-limits.sh" "$test_root/fixed.conf"
cp "$test_root/probe" "$test_root/mock/pnpm"
cat /proc/self/cgroup > "$test_root/expected"
echo "budget=$budget inode=$identity"
cat "/sys/fs/cgroup$budget/cpu.max" "/sys/fs/cgroup$budget/memory.max"
for command in cargo pnpm dev-run; do
  case $command in
    cargo) args=(test);;
    pnpm) args=(--version);;
    dev-run) args=(--frontend "$test_root/probe");;
  esac
  env PATH="$test_root/bin:$test_root/mock:$PATH"     ONEFLOWBASE_TASK_BUDGET_CGROUP="$budget" ONEFLOWBASE_TASK_BUDGET_ID="$identity"     RESOURCE_LIMITS_PROBE_LOG="$test_root/$command.log"     "$test_root/bin/$command" "${args[@]}"
  cmp "$test_root/expected" "$test_root/$command.log"
  printf '%s retained: ' "$command"; cat "$test_root/$command.log"
  for bad_context in stale missing outside; do
    bad_id=$identity
    bad_budget=$budget
    case $bad_context in
      stale) bad_id=1;;
      missing) bad_id=;;
      outside) bad_budget=/nonexistent-task-budget;;
    esac
    set +e
    env PATH="$test_root/bin:$test_root/mock:$PATH"       CARGO_MEMORY_BUDGET_ACTIVE=1       ONEFLOWBASE_TASK_BUDGET_CGROUP="$bad_budget" ONEFLOWBASE_TASK_BUDGET_ID="$bad_id"       RESOURCE_LIMITS_PROBE_LOG="$test_root/rejected.log"       "$test_root/bin/$command" "${args[@]}" 2>"$test_root/rejected.err"
    status=$?
    set -e
    [[ $status == 2 && ! -e "$test_root/rejected.log" ]]
    echo "$command rejected $bad_context context before legacy bypass"
  done
done
# A valid context replaces the wrapper with its own child. Cancellation may
# terminate that child, but must leave an unrelated sibling in the task alive.
sleep 30 &
sibling=$!
env ONEFLOWBASE_TASK_BUDGET_CGROUP="$budget" ONEFLOWBASE_TASK_BUDGET_ID="$identity"   RESOURCE_LIMITS_PROBE_LOG="$test_root/cancel.log" RESOURCE_LIMITS_PROBE_SLEEP=1   "$test_root/bin/cargo" test &
own=$!
for ((attempt=0;attempt<100;attempt++)); do
  [[ -s $test_root/cancel.log.pid ]] && break
  sleep 0.02
done
[[ -s $test_root/cancel.log.pid ]]
kill -TERM "$own"
set +e
wait "$own"
status=$?
set -e
[[ $status == 143 ]]
own=
kill -0 "$sibling"
kill -TERM "$sibling"
wait "$sibling" 2>/dev/null || true
sibling=
echo 'task cancellation left unrelated sibling alive'
echo 'resource budget context integration tests passed'
