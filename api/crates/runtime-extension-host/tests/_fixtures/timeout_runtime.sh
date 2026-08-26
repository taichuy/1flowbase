#!/usr/bin/env bash
set -euo pipefail

runtime_dir="$(cd -- "$(dirname -- "$0")" && pwd)"
printf '%s' $$ > "${runtime_dir}/runtime.pid"
sleep 5
