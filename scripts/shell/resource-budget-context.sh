#!/usr/bin/env bash
# Shared by installed wrappers. Environment is a declaration, never proof.
# Declare the absolute cgroup-v2 ancestor and its inode before starting a task.
oneflowbase_budget_validate() {
  local root=$1 current=$2 budget=$3 identity=$4 resolved quota period memory
  [[ $budget == /* && $budget != / && $identity =~ ^[1-9][0-9]*$ ]] || return 1
  resolved=$(realpath -e -- "$root$budget" 2>/dev/null) || return 1
  [[ $resolved == "$root$budget" ]] || return 1
  [[ $current == "$budget" || $current == "$budget/"* ]] || return 1
  [[ $(stat -c %i -- "$resolved") == "$identity" ]] || return 1
  read -r quota period < "$resolved/cpu.max" || return 1
  read -r memory < "$resolved/memory.max" || return 1
  [[ $quota =~ ^[1-9][0-9]*$ && $period =~ ^[1-9][0-9]*$ &&
     $memory =~ ^[1-9][0-9]*$ ]]
}

oneflowbase_task_budget_context() {
  [[ ${ONEFLOWBASE_TASK_BUDGET_CGROUP+x} || ${ONEFLOWBASE_TASK_BUDGET_ID+x} ]] || return 1
  local current
  current=$(sed -n 's/^0:://p' /proc/self/cgroup)
  if ! oneflowbase_budget_validate /sys/fs/cgroup "$current"       "${ONEFLOWBASE_TASK_BUDGET_CGROUP:-}" "${ONEFLOWBASE_TASK_BUDGET_ID:-}"; then
    printf 'Invalid or expired task resource budget context.\n' >&2
    return 2
  fi
}
