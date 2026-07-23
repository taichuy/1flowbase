#!/usr/bin/env bash
read _request
printf '%s\n' '{"type":"text_delta","delta":"hel"}'
printf '%s\n' '{"type":"text_delta","delta":"lo"}'
printf '%s\n' '{"type":"usage_snapshot","usage":{"input_tokens":2,"output_tokens":1,"total_tokens":3}}'
printf '%s\n' '{"type":"finish","reason":"stop"}'
printf '%s\n' '{"type":"result","result":{"final_content":"hello","usage":{"input_tokens":2,"output_tokens":1,"total_tokens":3},"finish_reason":"stop"}}'
