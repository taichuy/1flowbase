#!/usr/bin/env bash
read _request
printf '%s\n' '{"type":"result","result":{"final_content":"within-default-budget","finish_reason":"stop"}}'
