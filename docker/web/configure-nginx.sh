#!/bin/sh
set -eu

# Arguments are used by the deployment fixture; the image uses these defaults.
config_root=${1:-/config/nginx}
template=${2:-/usr/local/share/1flowbase/nginx.conf}
active=${3:-/etc/nginx/conf.d/default.conf}
generated=$config_root/nginx.default.conf
override=$config_root/nginx.conf

fail() {
  printf '1flowbase nginx: %s\n' "$1" >&2
  exit 1
}

mkdir -p "$config_root"
[ ! -d "$generated" ] || fail "$generated must be a file, not a directory"
temporary=$(mktemp "$config_root/.nginx.default.XXXXXX")
trap 'rm -f "$temporary"' 0
cp "$template" "$temporary"
chmod 644 "$temporary"
mv -f "$temporary" "$generated"

# Older Compose files mount the active file directly. Preserve that explicit
# operator choice, including read-only mounts, until the deployment migrates.
if awk -v target="$active" '$2 == target { found=1 } END { exit !found }' /proc/mounts; then
  printf '1flowbase nginx: retaining legacy mounted configuration %s; migrate to %s for managed defaults\n' "$active" "$config_root"
else
  selected=$template
  if [ -e "$override" ] || [ -L "$override" ]; then
    [ -f "$override" ] && [ -r "$override" ] || fail "$override must be a readable file; remove the incorrect path to use image defaults"
    selected=$override
    printf '1flowbase nginx: using persistent override %s; compare with %s after upgrades\n' "$override" "$generated"
  else
    printf '1flowbase nginx: using image defaults; editable template exported to %s\n' "$generated"
  fi
  cp "$selected" "$active"
fi

# Fail before serving requests when a custom configuration is invalid.
nginx -t
