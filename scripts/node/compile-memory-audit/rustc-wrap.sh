#!/bin/bash
# Original GNU time fields and descendant scope; no compiler/linker flag injection.
set -eu
source "${ONEFLOWBASE_AUDIT_BUDGET_HELPER:-$(dirname -- "$0")/resource-budget-context.sh}"
oneflowbase_task_budget_context
: "${RUSTC_MEM_LOG:?measurement log is required}"
crate=""; prev=""; kind=""; src=""; feat=""
for a in "$@"; do
  if [ "$prev" = "--crate-name" ]; then crate="$a"; fi
  if [ "$prev" = "--crate-type" ]; then kind="${kind}${a},"; fi
  if [ "$prev" = "--cfg" ]; then case "$a" in feature=*) feat="${feat}${a#feature=},";; esac; fi
  case "$a" in --test) kind="${kind}test,";; *.rs) [ -z "$src" ] && src="$a";; esac
  prev="$a"
done
if [ -z "$crate" ] || [ "$crate" = "___" ]; then exec "$@"; fi
exec /usr/bin/time -a -o "$RUSTC_MEM_LOG" -f "%M\t%e\t%U\t%S\t${crate}\t${kind}\t${src}\t${feat}" "$@"
