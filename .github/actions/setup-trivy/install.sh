#!/usr/bin/env bash
set -euo pipefail

version="${TRIVY_VERSION:?TRIVY_VERSION is required}"
if [[ ! "$version" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Expected an exact stable Trivy release tag, got: $version" >&2
  exit 1
fi
if [[ "$(uname -s)" != Linux ]]; then
  echo "Trivy installation currently supports Linux runners only" >&2
  exit 1
fi
case "$(uname -m)" in
  x86_64) arch=64bit ;;
  aarch64|arm64) arch=ARM64 ;;
  *) echo "Unsupported Trivy runner architecture" >&2; exit 1 ;;
esac

download_dir=$(mktemp -d "${RUNNER_TEMP:?}/trivy-download.XXXXXX")
trap 'rm -rf "$download_dir"' EXIT
asset="trivy_${version#v}_Linux-${arch}.tar.gz"
checksums="trivy_${version#v}_checksums.txt"
release_url="https://github.com/aquasecurity/trivy/releases/download/$version"
for file in "$asset" "$checksums"; do
  curl --fail --show-error --silent --location \
    --retry 3 --retry-all-errors --connect-timeout 10 --max-time 120 \
    --output "$download_dir/$file" "$release_url/$file"
done

(
  cd "$download_dir"
  awk -v asset="$asset" '$2 == asset { print }' "$checksums" | sha256sum --check --strict -
  tar -xzf "$asset" trivy
)
install_dir="$RUNNER_TEMP/trivy-$version/bin"
mkdir -p "$install_dir"
install -m 755 "$download_dir/trivy" "$install_dir/trivy"
"$install_dir/trivy" --version
printf '%s\n' "$install_dir" >> "${GITHUB_PATH:?}"
