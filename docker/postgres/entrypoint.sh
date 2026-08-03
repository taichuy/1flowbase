#!/bin/sh
set -eu

legacy_version_file=/var/lib/postgresql/PG_VERSION
if [ -f "$legacy_version_file" ]; then
  legacy_major="$(tr -d '[:space:]' < "$legacy_version_file")"
  echo >&2 "legacy PostgreSQL data layout detected (major ${legacy_major:-unknown}); an explicit major upgrade is required before PostgreSQL 18 can start"
  exit 1
fi

exec /usr/local/bin/docker-entrypoint.sh "$@"
