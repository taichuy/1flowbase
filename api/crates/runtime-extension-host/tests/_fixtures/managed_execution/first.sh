#!/bin/sh
payload=$(cat)
printf '{"ok":true,"result":{"worker":"first","request":%s}}\n' "$payload"
