#!/usr/bin/env sh
set -eu

FLOWBASE_REPO="${FLOWBASE_REPO:-taichuy/1flowbase}"
FLOWBASE_REF="${FLOWBASE_REF:-main}"
FLOWBASE_ARCHIVE_URL="${FLOWBASE_ARCHIVE_URL:-https://codeload.github.com/${FLOWBASE_REPO}/tar.gz/refs/heads/${FLOWBASE_REF}}"
FLOWBASE_ARCHIVE_DOCKER_DIR="1flowbase-${FLOWBASE_REF}/docker"
DEFAULT_OFFICIAL_PLUGIN_GITHUB_PROXY_URL="https://gh-proxy.com/"
DB_PASSWORD="${FLOWBASE_DB_PASSWORD:-}"
ROOT_ACCOUNT="${FLOWBASE_ROOT_ACCOUNT:-}"
ROOT_PASSWORD="${FLOWBASE_ROOT_PASSWORD:-}"
PROVIDER_SECRET="${FLOWBASE_PROVIDER_SECRET:-}"
WEB_PORT="${FLOWBASE_WEB_PORT:-}"
COOKIE_SECURE="${FLOWBASE_COOKIE_SECURE:-${API_COOKIE_SECURE:-}}"
DATABASE_MODE="${FLOWBASE_DATABASE_MODE:-}"
EXTERNAL_POSTGRES_HOST="${FLOWBASE_EXTERNAL_POSTGRES_HOST:-}"
EXTERNAL_POSTGRES_PORT="${FLOWBASE_EXTERNAL_POSTGRES_PORT:-}"
EXTERNAL_POSTGRES_DB="${FLOWBASE_EXTERNAL_POSTGRES_DB:-}"
EXTERNAL_POSTGRES_USER="${FLOWBASE_EXTERNAL_POSTGRES_USER:-}"
EXTERNAL_POSTGRES_PASSWORD="${FLOWBASE_EXTERNAL_POSTGRES_PASSWORD:-}"
EXTERNAL_POSTGRES_SSLMODE="${FLOWBASE_EXTERNAL_POSTGRES_SSLMODE:-}"
PLUGIN_GITHUB_PROXY_URL="${FLOWBASE_OFFICIAL_PLUGIN_GITHUB_PROXY_URL:-${API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL:-}}"
OFFICIAL_PLUGIN_SIGNATURE_REQUIRED="${FLOWBASE_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED:-${API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED:-}}"
PULL_IMAGES="${FLOWBASE_PULL_IMAGES:-}"
START_CONTAINERS="${FLOWBASE_START_CONTAINERS:-}"
INTERACTIVE=1

if [ "${FLOWBASE_NON_INTERACTIVE:-}" = "1" ] || [ "${FLOWBASE_NON_INTERACTIVE:-}" = "true" ]; then
  INTERACTIVE=0
fi

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

read_env_value() {
  key="$1"
  file="$2"
  if [ -f "$file" ]; then
    grep -E "^${key}=" "$file" | tail -n 1 | cut -d= -f2- || true
  fi
}

set_env_value() {
  key="$1"
  value="$2"
  file="$3"
  tmp="${file}.tmp.$$"
  awk -v key="$key" -v value="$value" '
    BEGIN { found = 0 }
    $0 ~ "^" key "=" {
      print key "=" value
      found = 1
      next
    }
    { print }
    END {
      if (!found) {
        print key "=" value
      }
    }
  ' "$file" > "$tmp" && mv "$tmp" "$file"
}

display_env_value() {
  key="$1"
  value="$2"
  if [ -z "$value" ]; then
    printf '%s\n' "<empty>"
    return
  fi

  case "$key" in
    POSTGRES_PASSWORD|EXTERNAL_POSTGRES_PASSWORD|API_DATABASE_URL|BOOTSTRAP_ROOT_PASSWORD|API_PROVIDER_SECRET_MASTER_KEY)
      printf '%s\n' "<set>"
      ;;
    *)
      printf '%s\n' "$value"
      ;;
  esac
}

print_env_summary() {
  file="$1"
  echo "Current docker/.env configuration:"
  for key in \
    FLOWBASE_WEB_VERSION \
    FLOWBASE_API_SERVER_VERSION \
    FLOWBASE_PLUGIN_RUNNER_VERSION \
    WEB_PORT \
    DATABASE_MODE \
    POSTGRES_DB \
    POSTGRES_USER \
    POSTGRES_PASSWORD \
    EXTERNAL_POSTGRES_HOST \
    EXTERNAL_POSTGRES_PORT \
    EXTERNAL_POSTGRES_DB \
    EXTERNAL_POSTGRES_USER \
    EXTERNAL_POSTGRES_PASSWORD \
    EXTERNAL_POSTGRES_SSLMODE \
    BOOTSTRAP_ROOT_ACCOUNT \
    BOOTSTRAP_ROOT_PASSWORD \
    API_PROVIDER_SECRET_MASTER_KEY \
    API_COOKIE_SECURE \
    API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL \
    API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED
  do
    value="$(read_env_value "$key" "$file")"
    echo "  ${key}=$(display_env_value "$key" "$value")"
  done
}

require_value() {
  option="$1"
  [ -n "${2-}" ] || fail "Missing value for ${option}."
  printf '%s\n' "$2"
}

normalize_true_false_value() {
  option="$1"
  value="$2"
  case "$value" in
    true|TRUE|True|1|yes|YES|Yes|y|Y)
      printf '%s\n' "true"
      ;;
    false|FALSE|False|0|no|NO|No|n|N)
      printf '%s\n' "false"
      ;;
    *)
      fail "Invalid value for ${option}: ${value}. Use true or false."
      ;;
  esac
}

normalize_database_mode() {
  option="$1"
  value="$2"
  case "$value" in
    ""|1|internal|INTERNAL|Internal)
      printf '%s\n' "internal"
      ;;
    2|external|EXTERNAL|External)
      printf '%s\n' "external"
      ;;
    *)
      fail "Invalid value for ${option}: ${value}. Use internal or external."
      ;;
  esac
}

normalize_postgres_sslmode() {
  option="$1"
  value="$2"
  [ -n "$value" ] || value="prefer"
  value="$(printf '%s' "$value" | tr '[:upper:]' '[:lower:]')"
  case "$value" in
    disable|allow|prefer|require|verify-ca|verify-full)
      printf '%s\n' "$value"
      ;;
    *)
      fail "Invalid value for ${option}: ${value}. Use disable, allow, prefer, require, verify-ca, or verify-full."
      ;;
  esac
}

database_url_encode_value() {
  printf '%s' "$1" | sed \
    -e 's/%/%25/g' \
    -e 's/ /%20/g' \
    -e 's/@/%40/g' \
    -e 's/:/%3A/g' \
    -e 's|/|%2F|g' \
    -e 's/?/%3F/g' \
    -e 's/#/%23/g' \
    -e 's/&/%26/g' \
    -e 's/=/%3D/g' \
    -e 's/+/%2B/g'
}

ensure_database_mode_default() {
  file="$1"
  current_value="$(read_env_value DATABASE_MODE "$file")"
  if [ -z "$current_value" ]; then
    set_env_value DATABASE_MODE internal "$file"
    echo "Added DATABASE_MODE=internal to docker/.env."
  fi
}

ensure_external_postgres_defaults() {
  file="$1"
  [ -n "$(read_env_value EXTERNAL_POSTGRES_PORT "$file")" ] || set_env_value EXTERNAL_POSTGRES_PORT 5432 "$file"
  [ -n "$(read_env_value EXTERNAL_POSTGRES_DB "$file")" ] || set_env_value EXTERNAL_POSTGRES_DB 1flowbase "$file"
  [ -n "$(read_env_value EXTERNAL_POSTGRES_USER "$file")" ] || set_env_value EXTERNAL_POSTGRES_USER postgres "$file"
  [ -n "$(read_env_value EXTERNAL_POSTGRES_SSLMODE "$file")" ] || set_env_value EXTERNAL_POSTGRES_SSLMODE prefer "$file"
}

sync_external_database_url() {
  file="$1"
  ensure_external_postgres_defaults "$file"

  host="$(read_env_value EXTERNAL_POSTGRES_HOST "$file")"
  port="$(read_env_value EXTERNAL_POSTGRES_PORT "$file")"
  db_name="$(read_env_value EXTERNAL_POSTGRES_DB "$file")"
  db_user="$(read_env_value EXTERNAL_POSTGRES_USER "$file")"
  db_password="$(read_env_value EXTERNAL_POSTGRES_PASSWORD "$file")"
  sslmode="$(normalize_postgres_sslmode EXTERNAL_POSTGRES_SSLMODE "$(read_env_value EXTERNAL_POSTGRES_SSLMODE "$file")")"

  [ -n "$host" ] || fail "EXTERNAL_POSTGRES_HOST is required when DATABASE_MODE=external."
  case "$port" in
    ""|*[!0123456789]*)
      fail "EXTERNAL_POSTGRES_PORT must be a number."
      ;;
  esac
  [ -n "$db_name" ] || fail "EXTERNAL_POSTGRES_DB is required when DATABASE_MODE=external."
  [ -n "$db_user" ] || fail "EXTERNAL_POSTGRES_USER is required when DATABASE_MODE=external."
  [ -n "$db_password" ] || fail "EXTERNAL_POSTGRES_PASSWORD is required when DATABASE_MODE=external."

  set_env_value EXTERNAL_POSTGRES_SSLMODE "$sslmode" "$file"
  encoded_user="$(database_url_encode_value "$db_user")"
  encoded_password="$(database_url_encode_value "$db_password")"
  encoded_db_name="$(database_url_encode_value "$db_name")"
  set_env_value API_DATABASE_URL "postgres://${encoded_user}:${encoded_password}@${host}:${port}/${encoded_db_name}?sslmode=${sslmode}" "$file"
}

sync_database_configuration() {
  file="$1"
  mode="$(normalize_database_mode DATABASE_MODE "$(read_env_value DATABASE_MODE "$file")")"
  set_env_value DATABASE_MODE "$mode" "$file"

  if [ "$mode" = "external" ]; then
    sync_external_database_url "$file"
  else
    set_env_value API_DATABASE_URL "" "$file"
  fi
}

provider_secret_master_key_is_placeholder() {
  value="$(printf '%s' "$1" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')"
  case "$value" in
    ""|change-me-provider-secret-master-key|dev-provider-secret-master-key-unsafe)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

generate_provider_secret_master_key() {
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -hex 32
    return
  fi

  if [ -r /dev/urandom ] && command -v od >/dev/null 2>&1; then
    od -An -N32 -tx1 /dev/urandom | tr -d ' \n'
    printf '\n'
    return
  fi

  fail "Could not generate API_PROVIDER_SECRET_MASTER_KEY. Install openssl or set FLOWBASE_PROVIDER_SECRET / --provider-secret."
}

ensure_provider_secret_master_key() {
  current_value="$(read_env_value API_PROVIDER_SECRET_MASTER_KEY ./docker/.env)"
  if provider_secret_master_key_is_placeholder "$current_value"; then
    generated_value="$(generate_provider_secret_master_key)"
    set_env_value API_PROVIDER_SECRET_MASTER_KEY "$generated_value" ./docker/.env
    echo "Generated API_PROVIDER_SECRET_MASTER_KEY in docker/.env."
  fi
}

ensure_official_plugin_signature_required() {
  current_value="$(read_env_value API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED ./docker/.env)"
  if [ -z "$current_value" ]; then
    set_env_value API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED true ./docker/.env
    echo "Added API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED=true to docker/.env."
  fi
}

ensure_cookie_secure_default() {
  current_value="$(read_env_value API_COOKIE_SECURE ./docker/.env)"
  if [ -z "$current_value" ]; then
    set_env_value API_COOKIE_SECURE true ./docker/.env
    echo "Added API_COOKIE_SECURE=true to docker/.env."
  fi
}

read_from_tty() {
  if [ "$INTERACTIVE" -eq 1 ] && [ -r /dev/tty ]; then
    IFS= read -r value < /dev/tty || value=""
    printf '%s\n' "$value"
  else
    printf '%s\n' ""
  fi
}

prompt_env_value() {
  key="$1"
  label="$2"
  current_value="$(read_env_value "$key" ./docker/.env)"

  if [ -n "$current_value" ]; then
    printf '%s [%s]: ' "$label" "$current_value" > /dev/tty 2>/dev/null || true
  else
    printf '%s: ' "$label" > /dev/tty 2>/dev/null || true
  fi

  input="$(read_from_tty)"
  if [ -n "$input" ]; then
    set_env_value "$key" "$input" ./docker/.env
    echo "Updated ${key} in docker/.env."
  else
    echo "Keeping ${key}: ${current_value:-empty}"
  fi
}

prompt_secret_env_value() {
  key="$1"
  label="$2"
  current_value="$(read_env_value "$key" ./docker/.env)"

  if [ -n "$current_value" ]; then
    printf '%s [<set>]: ' "$label" > /dev/tty 2>/dev/null || true
  else
    printf '%s: ' "$label" > /dev/tty 2>/dev/null || true
  fi

  input="$(read_from_tty)"
  if [ -n "$input" ]; then
    set_env_value "$key" "$input" ./docker/.env
    echo "Updated ${key} in docker/.env."
  else
    if [ -n "$current_value" ]; then
      echo "Keeping ${key}: <set>"
    else
      echo "Keeping ${key}: empty"
    fi
  fi
}

prompt_true_false_env_value() {
  key="$1"
  label="$2"
  current_value="$(read_env_value "$key" ./docker/.env)"

  if [ -n "$current_value" ]; then
    printf '%s [%s]: ' "$label" "$current_value" > /dev/tty 2>/dev/null || true
  else
    printf '%s: ' "$label" > /dev/tty 2>/dev/null || true
  fi

  input="$(read_from_tty)"
  if [ -n "$input" ]; then
    normalized_value="$(normalize_true_false_value "$key" "$input")"
    set_env_value "$key" "$normalized_value" ./docker/.env
    echo "Updated ${key} in docker/.env."
  else
    echo "Keeping ${key}: ${current_value:-empty}"
  fi
}

prompt_database_configuration() {
  current_mode="$(normalize_database_mode DATABASE_MODE "$(read_env_value DATABASE_MODE ./docker/.env)")"
  if [ "$current_mode" = "external" ]; then
    default_choice=2
  else
    default_choice=1
  fi

  printf 'Database mode (1=internal PostgreSQL, 2=external PostgreSQL) [%s]: ' "$default_choice" > /dev/tty 2>/dev/null || true
  input="$(read_from_tty)"
  if [ -z "$input" ]; then
    input="$current_mode"
  fi
  mode="$(normalize_database_mode DATABASE_MODE "$input")"
  set_env_value DATABASE_MODE "$mode" ./docker/.env
  echo "Updated DATABASE_MODE=${mode} in docker/.env."

  if [ "$mode" = "external" ]; then
    ensure_external_postgres_defaults ./docker/.env
    prompt_env_value EXTERNAL_POSTGRES_HOST "External PostgreSQL host/IP"
    prompt_env_value EXTERNAL_POSTGRES_PORT "External PostgreSQL port"
    prompt_env_value EXTERNAL_POSTGRES_DB "External PostgreSQL database"
    prompt_env_value EXTERNAL_POSTGRES_USER "External PostgreSQL user"
    prompt_secret_env_value EXTERNAL_POSTGRES_PASSWORD "External PostgreSQL password"
    prompt_env_value EXTERNAL_POSTGRES_SSLMODE "External PostgreSQL sslmode"
    sync_external_database_url ./docker/.env
  else
    set_env_value API_DATABASE_URL "" ./docker/.env
    prompt_env_value POSTGRES_PASSWORD "Database password"
  fi
}

normalize_yes_no() {
  case "$1" in
    y|Y|yes|YES|Yes|true|TRUE|1)
      printf '%s\n' "yes"
      ;;
    *)
      printf '%s\n' "no"
      ;;
  esac
}

prompt_yes_no() {
  question="$1"
  default_answer="$2"
  if [ "$default_answer" = "yes" ]; then
    suffix="[Y/n]"
  else
    suffix="[y/N]"
  fi

  printf '%s %s: ' "$question" "$suffix" > /dev/tty 2>/dev/null || true
  input="$(read_from_tty)"
  if [ -z "$input" ]; then
    printf '%s\n' "$default_answer"
  else
    normalize_yes_no "$input"
  fi
}

prompt_official_plugin_github_proxy_url() {
  current_value="$(read_env_value API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL ./docker/.env)"
  default_answer="no"
  if [ -n "$current_value" ]; then
    default_answer="yes"
  fi

  enabled="$(prompt_yes_no "Use CN GitHub plugin download accelerator?" "$default_answer")"
  if [ "$enabled" = "yes" ]; then
    default_value="$current_value"
    if [ -z "$default_value" ]; then
      default_value="$DEFAULT_OFFICIAL_PLUGIN_GITHUB_PROXY_URL"
    fi

    printf 'Official plugin GitHub raw proxy URL [%s]: ' "$default_value" > /dev/tty 2>/dev/null || true
    input="$(read_from_tty)"
    if [ -n "$input" ]; then
      set_env_value API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL "$input" ./docker/.env
    else
      set_env_value API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL "$default_value" ./docker/.env
    fi
    echo "Updated API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL in docker/.env."
  else
    set_env_value API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL "" ./docker/.env
    echo "Disabled API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL in docker/.env."
  fi
}

postgres_data_exists() {
  [ -d ./docker/postgres/data/pgdata ] || return 1
  [ -f ./docker/postgres/data/pgdata/PG_VERSION ] && return 0
  find ./docker/postgres/data/pgdata -mindepth 1 -maxdepth 1 -print -quit 2>/dev/null | grep -q .
}

sql_quote_literal_value() {
  printf '%s' "$1" | sed "s/'/''/g"
}

sql_user_identifier() {
  value="$1"
  case "$value" in
    ""|*[!abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_]*)
      escaped="$(printf '%s' "$value" | sed 's/"/""/g')"
      printf '"%s"\n' "$escaped"
      ;;
    *)
      printf '%s\n' "$value"
      ;;
  esac
}

sync_postgres_password() {
  new_password="$1"
  db_name="$(read_env_value POSTGRES_DB .env)"
  db_user="$(read_env_value POSTGRES_USER .env)"
  [ -n "$db_name" ] || db_name="1flowbase"
  [ -n "$db_user" ] || db_user="postgres"

  escaped_password="$(sql_quote_literal_value "$new_password")"
  db_user_sql="$(sql_user_identifier "$db_user")"

  echo "Postgres password changed and existing pgdata was found; syncing database user password."
  compose up -d db

  ready=0
  attempt=1
  while [ "$attempt" -le 30 ]; do
    if compose exec -T db pg_isready -U "$db_user" -d "$db_name" >/dev/null 2>&1; then
      ready=1
      break
    fi
    sleep 2
    attempt=$((attempt + 1))
  done

  [ "$ready" -eq 1 ] || fail "Postgres did not become ready; could not sync POSTGRES_PASSWORD."

  compose exec -T db psql -U "$db_user" -d "$db_name" -v ON_ERROR_STOP=1 -c "ALTER USER ${db_user_sql} WITH PASSWORD '${escaped_password}';"
  echo "Synced Postgres user password for ${db_user}."
}

normalize_docker_architecture() {
  case "$1" in
    amd64|x86_64)
      printf '%s\n' "amd64"
      ;;
    arm64|aarch64|arm64/v8)
      printf '%s\n' "arm64"
      ;;
    *)
      printf '%s\n' "$1"
      ;;
  esac
}

normalize_docker_platform() {
  platform="$1"
  case "$platform" in
    */*)
      os_name="${platform%%/*}"
      arch_name="${platform#*/}"
      ;;
    *)
      os_name="linux"
      arch_name="$platform"
      ;;
  esac

  os_name="$(printf '%s' "$os_name" | tr '[:upper:]' '[:lower:]')"
  arch_name="$(normalize_docker_architecture "$(printf '%s' "$arch_name" | tr '[:upper:]' '[:lower:]')")"
  printf '%s/%s\n' "$os_name" "$arch_name"
}

detect_docker_platform() {
  if [ -n "${DOCKER_DEFAULT_PLATFORM:-}" ]; then
    normalize_docker_platform "$DOCKER_DEFAULT_PLATFORM"
    return
  fi

  platform="$(docker info --format '{{.OSType}}/{{.Architecture}}' 2>/dev/null || true)"
  [ -n "$platform" ] || fail "Could not detect Docker server platform."
  normalize_docker_platform "$platform"
}

flowbase_env_or_file_value() {
  key="$1"
  file="$2"
  default_value="$3"
  value=""

  case "$key" in
    FLOWBASE_WEB_VERSION)
      value="${FLOWBASE_WEB_VERSION:-}"
      ;;
    FLOWBASE_API_SERVER_VERSION)
      value="${FLOWBASE_API_SERVER_VERSION:-}"
      ;;
    FLOWBASE_PLUGIN_RUNNER_VERSION)
      value="${FLOWBASE_PLUGIN_RUNNER_VERSION:-}"
      ;;
  esac

  if [ -z "$value" ]; then
    value="$(read_env_value "$key" "$file")"
  fi
  if [ -z "$value" ]; then
    value="$default_value"
  fi

  printf '%s\n' "$value"
}

flowbase_web_image() {
  file="$1"
  version="$(flowbase_env_or_file_value FLOWBASE_WEB_VERSION "$file" latest)"
  printf '%s\n' "ghcr.io/taichuy/1flowbase-web:${version}"
}

flowbase_api_server_image() {
  file="$1"
  version="$(flowbase_env_or_file_value FLOWBASE_API_SERVER_VERSION "$file" latest)"
  printf '%s\n' "ghcr.io/taichuy/1flowbase-api-server:${version}"
}

flowbase_plugin_runner_image() {
  file="$1"
  version="$(flowbase_env_or_file_value FLOWBASE_PLUGIN_RUNNER_VERSION "$file" latest)"
  printf '%s\n' "ghcr.io/taichuy/1flowbase-plugin-runner:${version}"
}

flowbase_uses_latest_image_tags() {
  file="$1"
  [ "$(flowbase_env_or_file_value FLOWBASE_WEB_VERSION "$file" latest)" = "latest" ] || return 1
  [ "$(flowbase_env_or_file_value FLOWBASE_API_SERVER_VERSION "$file" latest)" = "latest" ] || return 1
  [ "$(flowbase_env_or_file_value FLOWBASE_PLUGIN_RUNNER_VERSION "$file" latest)" = "latest" ] || return 1
  return 0
}

local_flowbase_latest_images_exist() {
  file="$1"
  flowbase_uses_latest_image_tags "$file" || return 1
  docker image inspect "$(flowbase_web_image "$file")" >/dev/null 2>&1 || return 1
  docker image inspect "$(flowbase_api_server_image "$file")" >/dev/null 2>&1 || return 1
  docker image inspect "$(flowbase_plugin_runner_image "$file")" >/dev/null 2>&1 || return 1
  return 0
}

prompt_pull_images() {
  if local_flowbase_latest_images_exist ./docker/.env; then
    prompt_yes_no "Local latest Docker images were found. Update Docker images?" "no"
  else
    prompt_yes_no "Pull Docker images?" "no"
  fi
}

manifest_supports_platform() {
  image="$1"
  platform="$2"
  os_name="${platform%%/*}"
  arch_name="${platform#*/}"
  manifest="$(docker manifest inspect "$image" 2>/dev/null || true)"

  [ -n "$manifest" ] || return 2
  printf '%s\n' "$manifest" | grep -Eq "\"os\"[[:space:]]*:[[:space:]]*\"${os_name}\"" || return 1
  printf '%s\n' "$manifest" | grep -Eq "\"architecture\"[[:space:]]*:[[:space:]]*\"${arch_name}\"" || return 1
  return 0
}

verify_flowbase_image_platforms() {
  platform="$(detect_docker_platform)"
  echo "Detected Docker platform: ${platform}"

  case "$platform" in
    linux/amd64|linux/arm64)
      ;;
    *)
      fail "This 1flowbase Docker package supports linux/amd64 and linux/arm64. Detected Docker platform: ${platform}."
      ;;
  esac

  for image in \
    "$(flowbase_web_image .env)" \
    "$(flowbase_api_server_image .env)" \
    "$(flowbase_plugin_runner_image .env)"
  do
    manifest_status=0
    manifest_supports_platform "$image" "$platform" || manifest_status="$?"
    if [ "$manifest_status" -eq 0 ]; then
      continue
    fi

    if [ "$manifest_status" -eq 1 ]; then
      fail "Docker image ${image} does not publish ${platform}. Rebuild/publish the 1flowbase multi-platform images, or set DOCKER_DEFAULT_PLATFORM=linux/amd64 as a temporary workaround on ARM machines."
    fi

    echo "Could not verify Docker image platform support for ${image}; continuing to Docker pull."
  done
}

usage() {
  cat <<'EOF'
Usage: docker-deploy.sh [options]

Options:
  --db-password VALUE       Pre-fill POSTGRES_PASSWORD before the interactive prompt.
  --root-account VALUE      Pre-fill BOOTSTRAP_ROOT_ACCOUNT before the interactive prompt.
  --root-password VALUE     Pre-fill BOOTSTRAP_ROOT_PASSWORD before the interactive prompt.
  --provider-secret VALUE   Pre-fill API_PROVIDER_SECRET_MASTER_KEY before the interactive prompt.
  --web-port VALUE          Pre-fill WEB_PORT before the interactive prompt.
  --cookie-secure VALUE     Pre-fill API_COOKIE_SECURE as true or false.
  --database-mode VALUE     Use internal or external PostgreSQL. Defaults to internal.
  --external-postgres-host VALUE
                            Pre-fill EXTERNAL_POSTGRES_HOST for external PostgreSQL.
  --external-postgres-port VALUE
                            Pre-fill EXTERNAL_POSTGRES_PORT for external PostgreSQL.
  --external-postgres-db VALUE
                            Pre-fill EXTERNAL_POSTGRES_DB for external PostgreSQL.
  --external-postgres-user VALUE
                            Pre-fill EXTERNAL_POSTGRES_USER for external PostgreSQL.
  --external-postgres-password VALUE
                            Pre-fill EXTERNAL_POSTGRES_PASSWORD for external PostgreSQL.
  --external-postgres-sslmode VALUE
                            Pre-fill EXTERNAL_POSTGRES_SSLMODE for external PostgreSQL.
  --plugin-github-proxy-url VALUE
                            Pre-fill API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL before the interactive prompt.
  --official-plugin-signature-required VALUE
                            Pre-fill API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED as true or false.
  --pull                    Pull images without asking.
  --no-pull                 Do not pull images without asking.
  --start                   Start containers without asking.
  --no-start                Do not start containers without asking.
  --non-interactive         Do not prompt. Defaults to no pull and no start unless --pull/--start are set.
  -h, --help                Show this help.

Environment variables with the same effect:
  FLOWBASE_DB_PASSWORD
  FLOWBASE_ROOT_ACCOUNT
  FLOWBASE_ROOT_PASSWORD
  FLOWBASE_PROVIDER_SECRET
  FLOWBASE_WEB_PORT
  FLOWBASE_COOKIE_SECURE
  FLOWBASE_DATABASE_MODE
  FLOWBASE_EXTERNAL_POSTGRES_HOST
  FLOWBASE_EXTERNAL_POSTGRES_PORT
  FLOWBASE_EXTERNAL_POSTGRES_DB
  FLOWBASE_EXTERNAL_POSTGRES_USER
  FLOWBASE_EXTERNAL_POSTGRES_PASSWORD
  FLOWBASE_EXTERNAL_POSTGRES_SSLMODE
  FLOWBASE_OFFICIAL_PLUGIN_GITHUB_PROXY_URL
  FLOWBASE_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED
  FLOWBASE_PULL_IMAGES=1
  FLOWBASE_START_CONTAINERS=1
  FLOWBASE_NON_INTERACTIVE=1
  DOCKER_DEFAULT_PLATFORM=linux/amd64
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --db-password)
      DB_PASSWORD="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --db-password=*)
      DB_PASSWORD="${1#*=}"
      shift
      ;;
    --root-account)
      ROOT_ACCOUNT="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --root-account=*)
      ROOT_ACCOUNT="${1#*=}"
      shift
      ;;
    --root-password)
      ROOT_PASSWORD="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --root-password=*)
      ROOT_PASSWORD="${1#*=}"
      shift
      ;;
    --provider-secret)
      PROVIDER_SECRET="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --provider-secret=*)
      PROVIDER_SECRET="${1#*=}"
      shift
      ;;
    --web-port)
      WEB_PORT="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --web-port=*)
      WEB_PORT="${1#*=}"
      shift
      ;;
    --cookie-secure)
      COOKIE_SECURE="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --cookie-secure=*)
      COOKIE_SECURE="${1#*=}"
      shift
      ;;
    --database-mode)
      DATABASE_MODE="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --database-mode=*)
      DATABASE_MODE="${1#*=}"
      shift
      ;;
    --external-postgres-host)
      EXTERNAL_POSTGRES_HOST="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --external-postgres-host=*)
      EXTERNAL_POSTGRES_HOST="${1#*=}"
      shift
      ;;
    --external-postgres-port)
      EXTERNAL_POSTGRES_PORT="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --external-postgres-port=*)
      EXTERNAL_POSTGRES_PORT="${1#*=}"
      shift
      ;;
    --external-postgres-db)
      EXTERNAL_POSTGRES_DB="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --external-postgres-db=*)
      EXTERNAL_POSTGRES_DB="${1#*=}"
      shift
      ;;
    --external-postgres-user)
      EXTERNAL_POSTGRES_USER="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --external-postgres-user=*)
      EXTERNAL_POSTGRES_USER="${1#*=}"
      shift
      ;;
    --external-postgres-password)
      EXTERNAL_POSTGRES_PASSWORD="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --external-postgres-password=*)
      EXTERNAL_POSTGRES_PASSWORD="${1#*=}"
      shift
      ;;
    --external-postgres-sslmode)
      EXTERNAL_POSTGRES_SSLMODE="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --external-postgres-sslmode=*)
      EXTERNAL_POSTGRES_SSLMODE="${1#*=}"
      shift
      ;;
    --plugin-github-proxy-url)
      PLUGIN_GITHUB_PROXY_URL="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --plugin-github-proxy-url=*)
      PLUGIN_GITHUB_PROXY_URL="${1#*=}"
      shift
      ;;
    --official-plugin-signature-required)
      OFFICIAL_PLUGIN_SIGNATURE_REQUIRED="$(require_value "$1" "${2-}")"
      shift 2
      ;;
    --official-plugin-signature-required=*)
      OFFICIAL_PLUGIN_SIGNATURE_REQUIRED="${1#*=}"
      shift
      ;;
    --pull)
      PULL_IMAGES=1
      shift
      ;;
    --no-pull)
      PULL_IMAGES=0
      shift
      ;;
    --start)
      START_CONTAINERS=1
      shift
      ;;
    --no-start|--prepare-only)
      START_CONTAINERS=0
      shift
      ;;
    --non-interactive)
      INTERACTIVE=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "Unknown option: $1"
      ;;
  esac
done

command -v docker >/dev/null 2>&1 || fail "Docker is required. Install Docker Engine or Docker Desktop first."

if docker compose version >/dev/null 2>&1; then
  compose() { docker compose "$@"; }
elif command -v docker-compose >/dev/null 2>&1; then
  compose() { docker-compose "$@"; }
else
  fail "Docker Compose is required. Install the Docker Compose plugin or docker-compose first."
fi

if [ -d ./docker ]; then
  echo "Using existing ./docker directory."
else
  command -v tar >/dev/null 2>&1 || fail "tar is required to unpack the 1flowbase archive."
  if command -v curl >/dev/null 2>&1; then
    download() { curl -fsSL "$1" -o "$2"; }
  elif command -v wget >/dev/null 2>&1; then
    download() { wget -qO "$2" "$1"; }
  else
    fail "curl or wget is required to download the 1flowbase docker files."
  fi

  tmpdir="$(mktemp -d 2>/dev/null || mktemp -d -t 1flowbase)"
  trap 'rm -rf "$tmpdir"' EXIT HUP INT TERM
  archive="$tmpdir/1flowbase.tar.gz"
  echo "Downloading 1flowbase docker files."
  download "$FLOWBASE_ARCHIVE_URL" "$archive"
  tar -xzf "$archive" -C "$tmpdir" "$FLOWBASE_ARCHIVE_DOCKER_DIR"
  mv "$tmpdir/$FLOWBASE_ARCHIVE_DOCKER_DIR" ./docker
  echo "Downloaded ./docker."
fi

PROMPT_CONFIG_VALUES=0
OLD_POSTGRES_PASSWORD=""
OLD_BOOTSTRAP_ROOT_ACCOUNT=""
OLD_BOOTSTRAP_ROOT_PASSWORD=""
OLD_PROVIDER_SECRET=""
if [ -f ./docker/.env ]; then
  OLD_POSTGRES_PASSWORD="$(read_env_value POSTGRES_PASSWORD ./docker/.env)"
  OLD_BOOTSTRAP_ROOT_ACCOUNT="$(read_env_value BOOTSTRAP_ROOT_ACCOUNT ./docker/.env)"
  OLD_BOOTSTRAP_ROOT_PASSWORD="$(read_env_value BOOTSTRAP_ROOT_PASSWORD ./docker/.env)"
  OLD_PROVIDER_SECRET="$(read_env_value API_PROVIDER_SECRET_MASTER_KEY ./docker/.env)"
fi

if [ ! -f ./docker/.env ]; then
  cp ./docker/.env.example ./docker/.env
  echo "Created docker/.env from docker/.env.example."
  ensure_database_mode_default ./docker/.env
  ensure_official_plugin_signature_required
  ensure_cookie_secure_default
  PROMPT_CONFIG_VALUES=1
else
  echo "Using existing docker/.env."
  ensure_database_mode_default ./docker/.env
  ensure_official_plugin_signature_required
  ensure_cookie_secure_default
  if [ "$INTERACTIVE" -eq 1 ] && [ -r /dev/tty ]; then
    print_env_summary ./docker/.env
    UPDATE_ENV="$(prompt_yes_no "Update current docker/.env configuration?" "no")"
    if [ "$UPDATE_ENV" = "yes" ]; then
      PROMPT_CONFIG_VALUES=1
    else
      echo "Keeping existing docker/.env."
    fi
  fi
fi

if [ -n "$DB_PASSWORD" ]; then
  set_env_value POSTGRES_PASSWORD "$DB_PASSWORD" ./docker/.env
  echo "Updated POSTGRES_PASSWORD in docker/.env."
fi
if [ -n "$ROOT_ACCOUNT" ]; then
  set_env_value BOOTSTRAP_ROOT_ACCOUNT "$ROOT_ACCOUNT" ./docker/.env
  echo "Updated BOOTSTRAP_ROOT_ACCOUNT in docker/.env."
fi
if [ -n "$ROOT_PASSWORD" ]; then
  set_env_value BOOTSTRAP_ROOT_PASSWORD "$ROOT_PASSWORD" ./docker/.env
  echo "Updated BOOTSTRAP_ROOT_PASSWORD in docker/.env."
fi
if [ -n "$PROVIDER_SECRET" ]; then
  set_env_value API_PROVIDER_SECRET_MASTER_KEY "$PROVIDER_SECRET" ./docker/.env
  echo "Updated API_PROVIDER_SECRET_MASTER_KEY in docker/.env."
fi
if [ -n "$WEB_PORT" ]; then
  set_env_value WEB_PORT "$WEB_PORT" ./docker/.env
  echo "Updated WEB_PORT in docker/.env."
fi
if [ -n "$COOKIE_SECURE" ]; then
  COOKIE_SECURE="$(normalize_true_false_value API_COOKIE_SECURE "$COOKIE_SECURE")"
  set_env_value API_COOKIE_SECURE "$COOKIE_SECURE" ./docker/.env
  echo "Updated API_COOKIE_SECURE in docker/.env."
fi
if [ -n "$DATABASE_MODE" ]; then
  DATABASE_MODE="$(normalize_database_mode DATABASE_MODE "$DATABASE_MODE")"
  set_env_value DATABASE_MODE "$DATABASE_MODE" ./docker/.env
  echo "Updated DATABASE_MODE in docker/.env."
fi
if [ -n "$EXTERNAL_POSTGRES_HOST" ]; then
  set_env_value EXTERNAL_POSTGRES_HOST "$EXTERNAL_POSTGRES_HOST" ./docker/.env
  echo "Updated EXTERNAL_POSTGRES_HOST in docker/.env."
fi
if [ -n "$EXTERNAL_POSTGRES_PORT" ]; then
  set_env_value EXTERNAL_POSTGRES_PORT "$EXTERNAL_POSTGRES_PORT" ./docker/.env
  echo "Updated EXTERNAL_POSTGRES_PORT in docker/.env."
fi
if [ -n "$EXTERNAL_POSTGRES_DB" ]; then
  set_env_value EXTERNAL_POSTGRES_DB "$EXTERNAL_POSTGRES_DB" ./docker/.env
  echo "Updated EXTERNAL_POSTGRES_DB in docker/.env."
fi
if [ -n "$EXTERNAL_POSTGRES_USER" ]; then
  set_env_value EXTERNAL_POSTGRES_USER "$EXTERNAL_POSTGRES_USER" ./docker/.env
  echo "Updated EXTERNAL_POSTGRES_USER in docker/.env."
fi
if [ -n "$EXTERNAL_POSTGRES_PASSWORD" ]; then
  set_env_value EXTERNAL_POSTGRES_PASSWORD "$EXTERNAL_POSTGRES_PASSWORD" ./docker/.env
  echo "Updated EXTERNAL_POSTGRES_PASSWORD in docker/.env."
fi
if [ -n "$EXTERNAL_POSTGRES_SSLMODE" ]; then
  EXTERNAL_POSTGRES_SSLMODE="$(normalize_postgres_sslmode EXTERNAL_POSTGRES_SSLMODE "$EXTERNAL_POSTGRES_SSLMODE")"
  set_env_value EXTERNAL_POSTGRES_SSLMODE "$EXTERNAL_POSTGRES_SSLMODE" ./docker/.env
  echo "Updated EXTERNAL_POSTGRES_SSLMODE in docker/.env."
fi
if [ -n "$PLUGIN_GITHUB_PROXY_URL" ]; then
  set_env_value API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL "$PLUGIN_GITHUB_PROXY_URL" ./docker/.env
  echo "Updated API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL in docker/.env."
fi
if [ -n "$OFFICIAL_PLUGIN_SIGNATURE_REQUIRED" ]; then
  OFFICIAL_PLUGIN_SIGNATURE_REQUIRED="$(normalize_true_false_value API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED "$OFFICIAL_PLUGIN_SIGNATURE_REQUIRED")"
  set_env_value API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED "$OFFICIAL_PLUGIN_SIGNATURE_REQUIRED" ./docker/.env
  echo "Updated API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED in docker/.env."
fi

if [ "$PROMPT_CONFIG_VALUES" -eq 1 ] && [ "$INTERACTIVE" -eq 1 ] && [ -r /dev/tty ]; then
  echo "Configure docker/.env. Press Enter to keep the value shown in brackets."
  prompt_database_configuration
  prompt_env_value BOOTSTRAP_ROOT_ACCOUNT "Root account"
  prompt_env_value BOOTSTRAP_ROOT_PASSWORD "Root password"
  prompt_env_value API_PROVIDER_SECRET_MASTER_KEY "API provider secret master key"
  prompt_env_value WEB_PORT "Web port"
  prompt_true_false_env_value API_COOKIE_SECURE "Use secure session cookies (true/false)"
  prompt_official_plugin_github_proxy_url
  prompt_true_false_env_value API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED "Require official plugin signatures (true/false)"
elif [ "$PROMPT_CONFIG_VALUES" -eq 1 ] && [ "$INTERACTIVE" -eq 1 ]; then
  echo "No interactive terminal was found. Keeping docker/.env values."
fi

ensure_provider_secret_master_key
sync_database_configuration ./docker/.env

NEW_POSTGRES_PASSWORD="$(read_env_value POSTGRES_PASSWORD ./docker/.env)"
NEW_BOOTSTRAP_ROOT_ACCOUNT="$(read_env_value BOOTSTRAP_ROOT_ACCOUNT ./docker/.env)"
NEW_BOOTSTRAP_ROOT_PASSWORD="$(read_env_value BOOTSTRAP_ROOT_PASSWORD ./docker/.env)"
NEW_PROVIDER_SECRET="$(read_env_value API_PROVIDER_SECRET_MASTER_KEY ./docker/.env)"
NEW_DATABASE_MODE="$(read_env_value DATABASE_MODE ./docker/.env)"
POSTGRES_PASSWORD_SYNC_REQUIRED=0

if [ "$NEW_DATABASE_MODE" = "internal" ] && postgres_data_exists; then
  if [ -n "$NEW_POSTGRES_PASSWORD" ] && [ "$OLD_POSTGRES_PASSWORD" != "$NEW_POSTGRES_PASSWORD" ]; then
    POSTGRES_PASSWORD_SYNC_REQUIRED=1
  fi

  if [ -n "$OLD_BOOTSTRAP_ROOT_ACCOUNT" ] && [ "$OLD_BOOTSTRAP_ROOT_ACCOUNT" != "$NEW_BOOTSTRAP_ROOT_ACCOUNT" ]; then
    echo "Warning: BOOTSTRAP_ROOT_ACCOUNT only affects initial bootstrap; existing root users are not renamed automatically."
  fi
  if [ -n "$OLD_BOOTSTRAP_ROOT_PASSWORD" ] && [ "$OLD_BOOTSTRAP_ROOT_PASSWORD" != "$NEW_BOOTSTRAP_ROOT_PASSWORD" ]; then
    echo "Warning: BOOTSTRAP_ROOT_PASSWORD only affects initial bootstrap; existing root passwords are not reset automatically."
  fi
  if [ -n "$OLD_PROVIDER_SECRET" ] && [ "$OLD_PROVIDER_SECRET" != "$NEW_PROVIDER_SECRET" ]; then
    message="API_PROVIDER_SECRET_MASTER_KEY changed while existing pgdata was found. Existing provider/data-source secrets may become unreadable without a key rotation."
    if [ "$INTERACTIVE" -eq 1 ] && [ -r /dev/tty ]; then
      CONTINUE_PROVIDER_SECRET_CHANGE="$(prompt_yes_no "${message} Continue?" "no")"
      if [ "$CONTINUE_PROVIDER_SECRET_CHANGE" != "yes" ]; then
        set_env_value API_PROVIDER_SECRET_MASTER_KEY "$OLD_PROVIDER_SECRET" ./docker/.env
        fail "Restored the previous API_PROVIDER_SECRET_MASTER_KEY."
      fi
    else
      set_env_value API_PROVIDER_SECRET_MASTER_KEY "$OLD_PROVIDER_SECRET" ./docker/.env
      fail "$message"
    fi
  fi
fi

if [ -z "$PULL_IMAGES" ]; then
  if [ "$INTERACTIVE" -eq 1 ] && [ -r /dev/tty ]; then
    PULL_IMAGES="$(prompt_pull_images)"
  else
    PULL_IMAGES="no"
  fi
else
  PULL_IMAGES="$(normalize_yes_no "$PULL_IMAGES")"
fi

if [ -z "$START_CONTAINERS" ]; then
  if [ "$INTERACTIVE" -eq 1 ] && [ -r /dev/tty ]; then
    START_CONTAINERS="$(prompt_yes_no "Start 1flowbase now?" "no")"
  else
    START_CONTAINERS="no"
  fi
else
  START_CONTAINERS="$(normalize_yes_no "$START_CONTAINERS")"
fi

if [ "$POSTGRES_PASSWORD_SYNC_REQUIRED" -eq 0 ] && [ "$PULL_IMAGES" = "no" ] && [ "$START_CONTAINERS" = "no" ]; then
  echo "Docker files are ready in ./docker."
  echo "No images were pulled and no containers were started."
  if [ "$NEW_DATABASE_MODE" = "external" ]; then
    echo "To start later, run: cd docker && docker compose -f docker-compose.external-db.yaml pull && docker compose -f docker-compose.external-db.yaml up -d"
  else
    echo "To start later, run: cd docker && docker compose pull && docker compose up -d"
  fi
  exit 0
fi

docker info >/dev/null 2>&1 || fail "Docker is installed but the daemon is not reachable. Start Docker and try again."

cd docker

if [ "$POSTGRES_PASSWORD_SYNC_REQUIRED" -eq 1 ]; then
  sync_postgres_password "$NEW_POSTGRES_PASSWORD"
fi

if [ "$PULL_IMAGES" = "yes" ] || [ "$START_CONTAINERS" = "yes" ]; then
  verify_flowbase_image_platforms
fi

if [ "$PULL_IMAGES" = "yes" ]; then
  if [ "$NEW_DATABASE_MODE" = "external" ]; then
    compose -f docker-compose.external-db.yaml pull
  else
    compose pull
  fi
else
  echo "Skipping image pull."
fi

if [ "$START_CONTAINERS" = "yes" ]; then
  if [ "$NEW_DATABASE_MODE" = "external" ]; then
    compose -f docker-compose.external-db.yaml up -d
  else
    compose up -d
  fi
else
  echo "Skipping container startup."
  if [ "$NEW_DATABASE_MODE" = "external" ]; then
    echo "To start later, run: cd docker && docker compose -f docker-compose.external-db.yaml up -d"
  else
    echo "To start later, run: cd docker && docker compose up -d"
  fi
  exit 0
fi

web_port="$(read_env_value WEB_PORT .env)"
root_account="$(read_env_value BOOTSTRAP_ROOT_ACCOUNT .env)"
root_password="$(read_env_value BOOTSTRAP_ROOT_PASSWORD .env)"

echo "1flowbase is starting. Web: http://127.0.0.1:${web_port:-3100}"
echo "Initial root account: ${root_account:-root}"
echo "Initial root password: ${root_password:-1flowbase}"
