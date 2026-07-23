#!/usr/bin/env bash
set -euo pipefail

payload="$(cat)"
case "${payload}" in
  *'"method":"validate_config"'*)
    printf '%s' '{"ok":true,"result":{"ok":true,"sanitized":{"client_id":"***"}}}'
    ;;
  *'"method":"test_connection"'*)
    printf '%s' '{"ok":true,"result":{"status":"ok"}}'
    ;;
  *'"method":"discover_catalog"'*)
    printf '%s' '{"ok":true,"result":[{"resource_key":"contacts","display_name":"Contacts","resource_kind":"object","metadata":{}}]}'
    ;;
  *'"method":"describe_resource"'*)
    printf '%s' '{"ok":true,"result":{"resource_key":"contacts","primary_key":"id","fields":[],"supports_preview_read":true,"supports_import_snapshot":true,"metadata":{}}}'
    ;;
  *'"method":"preview_read"'*)
    printf '%s' '{"ok":true,"result":{"rows":[{"id":"1","email":"person@example.com"}],"next_cursor":null}}'
    ;;
  *'"method":"import_snapshot"'*)
    printf '%s' '{"ok":true,"result":{"rows":[{"id":"1","email":"person@example.com"}],"schema_version":"v1","metadata":{}}}'
    ;;
  *'"method":"list_records"'*)
    printf '%s' '{"ok":true,"result":{"rows":[{"id":"contact-1","email":"person@example.com"}],"next_cursor":null,"total_count":1,"metadata":{"method":"list_records"}}}'
    ;;
  *'"method":"get_record"'*)
    printf '%s' '{"ok":true,"result":{"record":{"id":"contact-1","email":"person@example.com"},"metadata":{"method":"get_record"}}}'
    ;;
  *'"method":"create_record"'*)
    printf '%s' '{"ok":true,"result":{"record":{"id":"contact-created","email":"created@example.com"},"metadata":{"method":"create_record"}}}'
    ;;
  *'"method":"update_record"'*)
    printf '%s' '{"ok":true,"result":{"record":{"id":"contact-1","email":"updated@example.com"},"metadata":{"method":"update_record"}}}'
    ;;
  *'"method":"delete_record"'*)
    printf '%s' '{"ok":true,"result":{"deleted":true,"metadata":{"method":"delete_record"}}}'
    ;;
  *)
    printf '%s' '{"ok":false,"error":{"message":"unknown method","provider_summary":null}}'
    exit 1
    ;;
esac
