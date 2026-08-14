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
    *'"method":"invoke"'*)
      printf '{"type":"result","result":{"final_content":"pid:%s","finish_reason":"stop"}}\n' "$$"
      ;;
    *)
      printf '{"ok":true,"result":{"pid":%s}}\n' "$$"
      ;;
  esac
done
