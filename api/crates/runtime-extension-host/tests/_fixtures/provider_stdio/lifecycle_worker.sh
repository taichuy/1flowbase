#!/usr/bin/env bash
set -euo pipefail

while IFS= read -r payload; do
  case "${payload}" in
    *'"mode":"crash"'*)
      exit 23
      ;;
    *'"mode":"slow"'*)
      sleep 0.20
      ;;
  esac

  case "${payload}" in
    *'"method":"transport_session"'*)
      printf '%s\n' '{"ok":true,"result":{"generation":7,"reused":true,"physical_state":"closed","connection_age_ms":42,"ttl_remaining_ms":0,"close_reason":"requested_drain","close_acknowledged":true}}'
      ;;
    *'"method":"invoke"'*)
      printf '{"type":"result","result":{"final_content":"pid:%s","finish_reason":"stop"}}\n' "$$"
      ;;
    *)
      printf '{"ok":true,"result":{"pid":%s}}\n' "$$"
      ;;
  esac
done
