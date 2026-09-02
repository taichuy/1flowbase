#!/bin/sh
set -eu

repository="${1:?official repository is required}"
reference="${2:?official repository reference is required}"
output_dir="${3:?output directory is required}"

case "${repository}" in
  *[!A-Za-z0-9._/-]*|'')
    echo "invalid official repository" >&2
    exit 1
    ;;
esac

checkout_dir="$(mktemp -d)"
trap 'rm -rf "${checkout_dir}"' EXIT

git -C "${checkout_dir}" init --quiet
git -C "${checkout_dir}" remote add origin "https://github.com/${repository}.git"
git -C "${checkout_dir}" config core.sparseCheckout true
mkdir -p "${checkout_dir}/.git/info"
printf '/model-pricing/@*/\n/model-pricing/catalog-source.json\n' \
  > "${checkout_dir}/.git/info/sparse-checkout"
git -C "${checkout_dir}" fetch --quiet --depth 1 origin "${reference}"
git -C "${checkout_dir}" checkout --quiet --detach FETCH_HEAD

resolved_commit="$(git -C "${checkout_dir}" rev-parse HEAD)"
source_root="${checkout_dir}/model-pricing"
test -f "${source_root}/catalog-source.json"

mkdir -p "${output_dir}"
cp "${source_root}/catalog-source.json" "${output_dir}/catalog-source.json"
for provider_dir in "${source_root}"/@*; do
  test -d "${provider_dir}" || continue
  cp -R "${provider_dir}" "${output_dir}/"
done

rule_file_count="$(find "${output_dir}" -type f -name pricing.json | wc -l | tr -d ' ')"
test "${rule_file_count}" -gt 0
catalog_version="$(jq -er '.catalog_version' "${output_dir}/catalog-source.json")"
directory_sha256="$(
  cd "${output_dir}"
  find . -type f -name pricing.json -print | sort | xargs sha256sum | sha256sum | awk '{print $1}'
)"
jq -n \
  --arg schema_version "1flowbase.model-pricing-bootstrap-receipt/v1" \
  --arg repository "${repository}" \
  --arg resolved_commit "${resolved_commit}" \
  --arg catalog_version "${catalog_version}" \
  --arg directory_sha256 "sha256:${directory_sha256}" \
  --argjson source_file_count "${rule_file_count}" \
  '{schema_version:$schema_version,repository:$repository,resolved_commit:$resolved_commit,catalog_version:$catalog_version,source_file_count:$source_file_count,directory_sha256:$directory_sha256}' \
  > "${output_dir}/receipt.json"
