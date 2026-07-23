#!/usr/bin/env bash
set -euo pipefail
count=0
while IFS= read -r _request; do
  count=$((count + 1))
  printf '%s\n' "{\"type\":\"text_delta\",\"delta\":\"turn-${count}\"}"
  printf '%s\n' "{\"type\":\"result\",\"result\":{\"final_content\":\"turn-${count}\",\"finish_reason\":\"stop\"}}"
done
