#!/usr/bin/env bash
set -euo pipefail

while IFS= read -r request; do
  if [[ "${request}" == *'"mode":"unknown_cancel"'* ]]; then
    printf '%s\n' '{"frame":"host_cancel","protocol":"runtime_host_call/v1","call_id":"missing"}'
    continue
  fi

  printf '%s\n' '{"frame":"host_call","protocol":"runtime_host_call/v1","call_id":"data-1","service":"plugin_data/v1","request":{"operations":[{"operation":"count","target":{"kind":"owned_collection","collection_code":"affinity"}}]}}'
  if [[ "${request}" == *'"mode":"crash"'* ]]; then
    exit 29
  fi
  if [[ "${request}" == *'"mode":"cancel"'* ]]; then
    printf '%s\n' '{"frame":"host_cancel","protocol":"runtime_host_call/v1","call_id":"data-1"}'
  fi
  if [[ "${request}" == *'"mode":"duplicate"'* ]]; then
    printf '%s\n' '{"frame":"host_call","protocol":"runtime_host_call/v1","call_id":"data-1","service":"plugin_data/v1","request":{"operations":[{"operation":"count","target":{"kind":"owned_collection","collection_code":"affinity"}}]}}'
    continue
  fi
  IFS= read -r host_result
  if [[ "${host_result}" != *'"frame":"host_result"'* ]] || [[ "${host_result}" != *'"call_id":"data-1"'* ]]; then
    exit 31
  fi
  printf '%s\n' '{"type":"result","result":{"final_content":"host-call-ok","finish_reason":"stop"}}'
done
