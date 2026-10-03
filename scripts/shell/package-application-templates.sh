#!/bin/sh
set -eu

repository="${1:?official repository is required}"
reference="${2:?immutable repository reference is required}"
output_dir="${3:?output directory is required}"

case "$reference" in
  *[!0-9a-f]*|'') echo 'application template reference must be a commit SHA' >&2; exit 1 ;;
esac
test "${#reference}" -eq 40
checkout_dir="$(mktemp -d)"
trap 'rm -rf "$checkout_dir"' EXIT

# A local checkout supports offline release verification using the same packager.
if test -d "$repository"; then
  git -C "$repository" archive "$reference" applications-demo | tar -x -C "$checkout_dir"
else
  case "$repository" in
    *[!A-Za-z0-9._/-]*|'') echo 'invalid official repository' >&2; exit 1 ;;
  esac
  git -C "$checkout_dir" init --quiet
  git -C "$checkout_dir" remote add origin "https://github.com/$repository.git"
  git -C "$checkout_dir" config core.sparseCheckout true
  mkdir -p "$checkout_dir/.git/info"
  printf '/applications-demo/\n' > "$checkout_dir/.git/info/sparse-checkout"
  git -C "$checkout_dir" fetch --quiet --depth 1 origin "$reference"
  git -C "$checkout_dir" checkout --quiet --detach FETCH_HEAD
  test "$(git -C "$checkout_dir" rev-parse HEAD)" = "$reference"
fi

mkdir -p "$output_dir"
count=0
for namespace in "$checkout_dir"/applications-demo/@*; do
  test -d "$namespace" || continue
  for template_dir in "$namespace"/*; do
    test -d "$template_dir" || continue
    source_file="$template_dir/template.json"
    test -f "$source_file"
    identity="$(basename "$namespace")/$(basename "$template_dir")"
    jq -e --arg identity "$identity" '
      .schema_version == "1flowbase.portable-template/v1" and
      .release.template_id == $identity and
      (.release.release_version | type == "number" and . >= 1 and . == floor) and
      (.release.name | type == "string" and length > 0) and
      (.pages | type == "array") and (.applications | type == "array") and
      (.data_models | type == "array") and (.plugins | type == "array")
    ' "$source_file" >/dev/null || {
      echo "invalid application template: $identity" >&2; exit 1;
    }
    mkdir -p "$output_dir/$identity"
    cp "$source_file" "$output_dir/$identity/template.json"
    count=$((count + 1))
  done
done
test "$count" -gt 0
jq -n --arg repository "$repository" --arg resolved_commit "$reference" \
  --argjson source_file_count "$count" \
  '{schema_version:"1flowbase.application-template-bootstrap-receipt/v1",repository:$repository,resolved_commit:$resolved_commit,source_file_count:$source_file_count}' \
  > "$output_dir/receipt.json"
