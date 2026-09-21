#!/usr/bin/env bash
set -euo pipefail

# Installed as ~/.local/bin/dev-run. Children inherit the aggregate budget.
slice=dev.slice
if [[ ${1:-} == --frontend ]]; then
  slice=dev-frontend.slice
  shift
fi
if (( $# == 0 )); then
  set -- "${SHELL:-/bin/bash}"
fi
if ! systemctl --user is-active default.target >/dev/null 2>&1; then
  printf 'Development resource limits unavailable: user systemd manager is not active.\n' >&2
  exit 2
fi
# Avoid moving descendants out of an existing, more specific development scope.
if { [[ $slice == dev.slice ]] && grep -q '/dev.slice/' /proc/self/cgroup; } ||
   grep -q "/dev.slice/$slice/" /proc/self/cgroup; then
  exec "$@"
fi
exec systemd-run --user --scope --collect --quiet --same-dir \
  --unit="dev-run-$$-$RANDOM.scope" \
  --slice="$slice" -- "$@"
