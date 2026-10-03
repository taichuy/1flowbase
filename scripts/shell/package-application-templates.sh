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
  git -C "$repository" archive "$reference" applications-demo scripts/application-template | tar -x -C "$checkout_dir"
else
  case "$repository" in
    *[!A-Za-z0-9._/-]*|'') echo 'invalid official repository' >&2; exit 1 ;;
  esac
  git -C "$checkout_dir" init --quiet
  git -C "$checkout_dir" remote add origin "https://github.com/$repository.git"
  git -C "$checkout_dir" config core.sparseCheckout true
  mkdir -p "$checkout_dir/.git/info"
  printf '/applications-demo/\n/scripts/application-template/\n' > "$checkout_dir/.git/info/sparse-checkout"
  git -C "$checkout_dir" fetch --quiet --depth 1 origin "$reference"
  git -C "$checkout_dir" checkout --quiet --detach FETCH_HEAD
  test "$(git -C "$checkout_dir" rev-parse HEAD)" = "$reference"
fi

# Build the same deterministic ZIP as the release publisher. A pinned commit is
# the build trust boundary; published bytes are additionally checked against it.
node --input-type=module - "$checkout_dir" "$output_dir" "$repository" "$reference" <<'NODE'
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
const [checkout, output, repository, reference] = process.argv.slice(2);
const { buildArchive, readPackage } = await import(pathToFileURL(path.join(checkout, 'scripts/application-template/archive.mjs')).href);
fs.mkdirSync(output, { recursive: true });
let count = 0;
const packages = [];
for (const namespace of fs.readdirSync(path.join(checkout, 'applications-demo')).filter(x => x.startsWith('@')).sort()) {
  for (const name of fs.readdirSync(path.join(checkout, 'applications-demo', namespace)).sort()) {
    const source = path.join(checkout, 'applications-demo', namespace, name);
    if (!fs.statSync(source).isDirectory()) continue;
    const identity = `${namespace}/${name}`;
    const pkg = await readPackage(source);
    if (pkg.release?.template_id !== identity || !Number.isSafeInteger(pkg.release.release_version) || pkg.release.release_version < 1) throw new Error(`invalid application template: ${identity}`);
    const bytes = await buildArchive(source);
    const checksum = `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
    const entryPath = path.join(source, 'catalog-entry.json');
    if (fs.existsSync(entryPath)) {
      const entry = JSON.parse(fs.readFileSync(entryPath, 'utf8'));
      if (entry.checksum !== checksum) throw new Error(`release archive checksum mismatch: ${identity}`);
      // A production build exercises the actual immutable release download too.
      if (!fs.existsSync(repository)) {
        const locator = entry.download_locator?.locator;
        if (typeof locator !== 'string' || !locator.startsWith('https://github.com/')) throw new Error('invalid release download locator');
        const downloaded = execFileSync('curl', ['--fail', '--silent', '--show-error', '--location', '--retry', '3', '--max-time', '180', locator], { maxBuffer: 64 * 1024 * 1024 });
        if (!bytes.equals(downloaded)) throw new Error(`published bytes differ from pinned source: ${identity}`);
      }
    } else if (!fs.existsSync(repository)) throw new Error(`application template release is not published: ${identity}`);
    const destination = path.join(output, identity);
    fs.mkdirSync(destination, { recursive: true });
    fs.writeFileSync(path.join(destination, 'template.zip'), bytes);
    packages.push({ template_id: identity, release_version: pkg.release.release_version, checksum });
    count++;
  }
}
if (!count) throw new Error('no application templates found');
fs.writeFileSync(path.join(output, 'receipt.json'), JSON.stringify({ schema_version: '1flowbase.application-template-bootstrap-receipt/v1', repository, resolved_commit: reference, source_file_count: count, packages }, null, 2) + '\n');
NODE
