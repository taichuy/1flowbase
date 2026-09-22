#!/usr/bin/env bash
set -euo pipefail

# Resolve the current Node installation without recursively selecting this shim.
wrapper_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
search_path=()
IFS=: read -r -a path_entries <<< "$PATH"
for entry in "${path_entries[@]}"; do
  [[ $entry == "$wrapper_dir" ]] || search_path+=("$entry")
done
real_pnpm=$(PATH=$(IFS=:; printf '%s' "${search_path[*]}") command -v pnpm) || {
  printf 'pnpm installation not found outside %s\n' "$wrapper_dir" >&2
  exit 127
}
gate_command=()
for arg in "$@"; do
  case $arg in
    build|build:*|test|test:*|typecheck|type-check|lint|check)
      gate_command=("$wrapper_dir/dev-heavy-run") ;;
  esac
done
# Watchers keep running indefinitely and must not own the finite-job slot.
for arg in "$@"; do
  case $arg in --watch|dev|start|*:watch) gate_command=(); break ;; esac
done
exec "${gate_command[@]}" "$wrapper_dir/dev-run" --frontend "$real_pnpm" "$@"
