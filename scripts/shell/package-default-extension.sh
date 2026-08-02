#!/bin/sh
set -eu

lock_file="${1:?lock manifest is required}"
target_arch="${2:?target architecture is required}"
output_dir="${3:?output directory is required}"

entry="$(jq -c --arg target "linux-${target_arch}" '.defaults[] | select(.bundled_path | contains($target))' "${lock_file}")"
test -n "${entry}"
url="$(printf '%s' "${entry}" | jq -r '.artifact_url')"
checksum="$(printf '%s' "${entry}" | jq -r '.checksum | sub("^sha256:"; "")')"
relative_path="$(printf '%s' "${entry}" | jq -r '.bundled_path')"
destination="${output_dir}/${relative_path#bootstrap/}"

mkdir -p "${output_dir}"
curl --fail --location --retry 3 --output "${destination}" "${url}"
printf '%s  %s\n' "${checksum}" "${destination}" | sha256sum -c >/dev/null
