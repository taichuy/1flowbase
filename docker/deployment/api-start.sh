#!/bin/sh
set -eu
set -a
. /config/.env
set +a
exec /usr/local/bin/api-server "$@"
