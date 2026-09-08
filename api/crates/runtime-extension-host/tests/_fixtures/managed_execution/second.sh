#!/bin/sh
payload=$(cat)
printf '{"ok":true,"result":{"worker":"second","request":%s}}\n' "$payload"
