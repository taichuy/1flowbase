#!/bin/sh
set -eu
umask 077

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

# Optional paths/owner arguments allow the same entrypoint to run in test fixtures.
config_root=${1:-/config}
postgres_root=${2:-/pgdata}
owner=${3:-$(stat -c '%u:%g' /home/flowbase)}
if [ "$#" -gt 0 ]; then
  [ "$#" -ge 3 ] || fail 'Expected configuration root, PostgreSQL root and owner'
  shift 3
else
  set -- /app/api/storage /app/api/plugins/packages \
    /app/api/plugins/installed /app/api/plugins/host-extension/dropins
fi
file=$config_root/.env

existing_database() {
  [ ! -f "$postgres_root/PG_VERSION" ] || return 0
  for directory in "$postgres_root"/*; do
    version=${directory##*/}
    case "$version" in ''|*[!0-9]*) continue ;; esac
    [ ! -f "$directory/docker/PG_VERSION" ] || return 0
  done
  return 1
}

random_hex() {
  value=$(od -v -An -N "$1" -tx1 /dev/urandom | tr -d ' \n')
  [ "${#value}" -eq "$(( $1 * 2 ))" ] || fail 'Could not generate a random secret'
  printf '%s' "$value"
}

shell_quote() {
  printf "'"
  printf '%s' "$1" | sed "s/'/'\\\\''/g"
  printf "'"
}

write_value() {
  printf '%s=' "$1"
  shell_quote "$2"
  printf '\n'
}

if [ ! -f "$file" ] && existing_database; then
  fail 'Existing database without deployment configuration; restore the original config'
fi
for directory in "$config_root" "$@"; do
  mkdir -p "$directory"
  chown "$owner" "$directory"
done

temporary=
password_temporary=
trap 'rm -f "$temporary" "$password_temporary"' 0
created=false
if [ ! -f "$file" ]; then
  password=${POSTGRES_PASSWORD:-$(random_hex 24)}
  master_key=${API_PROVIDER_SECRET_MASTER_KEY:-$(random_hex 32)}
  # Keep URI-unreserved bytes and encode delimiters and non-ASCII bytes.
  encoded_password=$(printf '%s' "$password" | od -v -An -tu1 | awk '{
    for (i = 1; i <= NF; i++) {
      byte = $i
      if ((byte >= 48 && byte <= 57) || (byte >= 65 && byte <= 90) ||
          (byte >= 97 && byte <= 122) || byte == 45 || byte == 46 || byte == 95 || byte == 126)
        printf "%c", byte
      else printf "%%%02X", byte
    }
  }')
  temporary=$(mktemp "$config_root/.env.XXXXXX")
  {
    write_value POSTGRES_PASSWORD "$password"
    write_value API_DATABASE_URL "postgres://postgres:$encoded_password@db:5432/1flowbase"
    write_value API_PROVIDER_SECRET_MASTER_KEY "$master_key"
    write_value BOOTSTRAP_ROOT_ACCOUNT "${BOOTSTRAP_ROOT_ACCOUNT:-root}"
    write_value BOOTSTRAP_ROOT_PASSWORD "${BOOTSTRAP_ROOT_PASSWORD:-change-me-root-password}"
  } > "$temporary"
  chown "$owner" "$temporary"
  # Publish the complete file without replacing another initializer's result.
  if ln "$temporary" "$file" 2>/dev/null; then
    created=true
  else
    [ -f "$file" ] || fail 'Could not publish deployment configuration'
  fi
fi

password_temporary=$(mktemp "$config_root/.postgres-password.XXXXXX")
# A clean environment prevents inherited defaults from masking an incomplete file.
env -i /bin/sh -ec '
  . "$1"
  : "${POSTGRES_PASSWORD:?Missing POSTGRES_PASSWORD in deployment configuration}"
  : "${API_DATABASE_URL:?Missing API_DATABASE_URL in deployment configuration}"
  : "${API_PROVIDER_SECRET_MASTER_KEY:?Missing API_PROVIDER_SECRET_MASTER_KEY in deployment configuration}"
  : "${BOOTSTRAP_ROOT_ACCOUNT:?Missing BOOTSTRAP_ROOT_ACCOUNT in deployment configuration}"
  : "${BOOTSTRAP_ROOT_PASSWORD:?Missing BOOTSTRAP_ROOT_PASSWORD in deployment configuration}"
  printf "%s" "$POSTGRES_PASSWORD"
' -- "$file" > "$password_temporary"
# PostgreSQL's official entrypoint consumes this root-readable secret via *_FILE.
mv -f "$password_temporary" "$config_root/postgres-password"

if [ "$created" = true ]; then
  printf 'Generated persistent configuration: %s\n' "$file"
else
  printf 'Reusing persistent configuration: %s\n' "$file"
fi
