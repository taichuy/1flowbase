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
    *'"mode":"recovery_error"'*)
      # #2085: the exact two-line shape the real openai worker emits on the Err
      # path - a typed error event carrying the recovery receipt in
      # provider_details, followed by the terminal result line.
      printf '%s\n' '{"type":"error","error":{"kind":"provider_transport_unavailable","message":"websocket closed by server before response.completed (code: 1011, reason: upstream websocket proxy failed)","provider_details":{"1flowbase_provider_recovery":{"attempt":0,"transport":"ai_native_websocket","transport_epoch":149,"commit_level":"terminal","disposition":"terminal_interruption","reason":"transport_disconnected"},"1flowbase_provider_recovery_original_error":{"kind":"provider_transport_unavailable","message":"websocket closed by server before response.completed (code: 1011, reason: upstream websocket proxy failed)"}}}}'
      printf '%s\n' '{"type":"result","result":{"final_content":null,"finish_reason":"error","provider_metadata":{}}}'
      ;;
    *'"method":"invoke"'*)
      printf '{"type":"result","result":{"final_content":"pid:%s","finish_reason":"stop"}}\n' "$$"
      ;;
    *)
      printf '{"ok":true,"result":{"pid":%s}}\n' "$$"
      ;;
  esac
done
