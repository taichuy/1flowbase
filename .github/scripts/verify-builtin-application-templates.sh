#!/usr/bin/env bash
set -euo pipefail

image=${1:?usage: verify-builtin-application-templates.sh API_IMAGE}
postgres_image=postgres:18.4-alpine
fixture="flowbase-template-bootstrap-$$"
network="$fixture"
db="$fixture-db"
api="$fixture-api"

cleanup() {
  local status=$?
  if (( status != 0 )); then
    docker logs --tail 80 "$api" 2>&1 | tail -c 12000 >&2 || true
  fi
  docker rm -fv "$api" "$db" >/dev/null 2>&1 || true
  docker network rm "$network" >/dev/null 2>&1 || true
  return "$status"
}
trap cleanup EXIT

fail() { printf 'Built-in template gate: %s\n' "$1" >&2; exit 1; }
wait_for() {
  local deadline=$((SECONDS + $1))
  shift
  until timeout 5s "$@" >/dev/null 2>&1; do
    (( SECONDS < deadline )) || return 1
    sleep 1
  done
}

# Fail closed even if a build stopped shipping its template or receipt entirely.
receipt=$(docker run --rm --pull=never --network none --entrypoint cat "$image" \
  /app/api/resources/application-templates/receipt.json)
expected=$(jq -er '
  select(.schema_version == "1flowbase.application-template-bootstrap-receipt/v1")
  | select(.source_file_count == (.packages | length))
  | .packages
  | select(any(.template_id == "@taichuy/gateway-demo"))
  | .[]
  | select(.template_id | test("^@[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$"))
  | select(.release_version > 0 and (.release_version | floor) == .release_version)
  | [.template_id, .release_version] | @tsv
' <<< "$receipt") || fail 'missing required bundled template or invalid receipt'
count=$(jq -r '.source_file_count' <<< "$receipt")
[[ $(wc -l <<< "$expected") -eq "$count" ]] || fail 'invalid template identity or release in receipt'

docker network create --internal "$network" >/dev/null
docker run -d --name "$db" --network "$network" \
  -e POSTGRES_PASSWORD=template-fixture-password -e POSTGRES_DB=template_bootstrap \
  "$postgres_image" >/dev/null
wait_for 60 docker exec "$db" pg_isready -h 127.0.0.1 -U postgres -d template_bootstrap \
  || fail 'fresh PostgreSQL did not become ready'

# The image must use its own bundle and the default auto-update behavior.
# No provider setup, mounts, external API access, or published ports.
docker run -d --pull=never --name "$api" --network "$network" \
  -e API_ENV=production -e API_SERVER_ADDR=0.0.0.0:7800 \
  -e API_ALLOWED_ORIGINS=http://localhost:3100 \
  -e "API_DATABASE_URL=postgres://postgres:template-fixture-password@$db:5432/template_bootstrap" \
  -e API_PROVIDER_SECRET_MASTER_KEY=template-fixture-not-a-production-secret \
  -e BOOTSTRAP_ROOT_ACCOUNT=root -e BOOTSTRAP_ROOT_EMAIL=root@example.test \
  -e BOOTSTRAP_ROOT_PASSWORD=Template-fixture-only-39! -e BOOTSTRAP_WORKSPACE_NAME=1flowbase \
  "$image" >/dev/null
wait_for 120 docker exec "$api" node -e '
  fetch("http://127.0.0.1:7800/health", {signal: AbortSignal.timeout(1500)})
    .then(r => { if (!r.ok) process.exitCode = 1; }, () => { process.exitCode = 1; });
' || fail 'API did not become healthy on an empty database'

query() {
  timeout 10s docker exec "$db" psql -XAt -F $'\t' -v ON_ERROR_STOP=1 -U postgres -d template_bootstrap -c "$1"
}
installed=$(query 'SELECT template_id, release_version FROM application_template_releases WHERE successful')
if ! diff -u <(LC_ALL=C sort <<< "$expected") <(LC_ALL=C sort <<< "$installed"); then
  fail 'API was healthy but did not initialize every bundled template at the expected release'
fi
[[ $(query "SELECT EXISTS (
    SELECT 1 FROM frontstage_pages p
    JOIN application_template_releases r ON r.workspace_id = p.workspace_id
    WHERE r.template_id = '@taichuy/gateway-demo' AND r.successful AND p.slug = 'route'
  )") == t ]] || fail 'Gateway Demo route was not initialized'
[[ $(query 'SELECT count(*) FROM model_provider_instances') == 0 ]] || fail 'unexpected model provider instances'
printf 'Built-in application templates initialized without model providers:\n%s\n' "$installed"
