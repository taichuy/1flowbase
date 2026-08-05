const test = require('node:test');
const assert = require('node:assert/strict');
const childProcess = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');

test('AC-BOOT-1 and AC-BOOT-2 lock exact default artifacts into the API image', () => {
  const lockPath = path.join(repoRoot, 'api', 'plugins', 'default-extensions.lock.json');
  const lock = JSON.parse(fs.readFileSync(lockPath, 'utf8'));
  assert.equal(lock.schema_version, '1flowbase.extension-bootstrap-lock/v1');
  assert.deepEqual(
    lock.defaults.map(({ category, artifact_kind, id, version, source, bootstrap }) => ({
      category, artifact_kind, id, version, source, bootstrap,
    })),
    [
      {
        category: 'runtime_extensions', artifact_kind: 'model_provider',
        id: '1flowbase.anthropic', version: '0.1.33',
        source: 'official_registry', bootstrap: true,
      },
      {
        category: 'runtime_extensions', artifact_kind: 'model_provider',
        id: '1flowbase.anthropic', version: '0.1.33',
        source: 'official_registry', bootstrap: true,
      },
    ],
  );
  for (const entry of lock.defaults) {
    assert.match(entry.checksum, /^sha256:[a-f0-9]{64}$/u);
    assert.match(entry.artifact_url, /^https:\/\/github\.com\/taichuy\//u);
    assert.match(entry.bundled_path, /^bootstrap\//u);
  }

  const dockerfile = fs.readFileSync(path.join(repoRoot, 'docker', 'api-server.Dockerfile'), 'utf8');
  assert.match(dockerfile, /package-default-extension \/tmp\/default-extensions\.lock\.json/u);
  assert.match(
    dockerfile,
    /COPY --from=default-extension \/default-extensions \/app\/api\/plugins\/bootstrap/u,
  );

  const packager = fs.readFileSync(
    path.join(repoRoot, 'scripts', 'shell', 'package-default-extension.sh'),
    'utf8',
  );
  assert.match(packager, /sha256sum -c/u);
  assert.doesNotMatch(packager, /sha256sum --check --status/u);

  const workflow = fs.readFileSync(
    path.join(repoRoot, '.github', 'workflows', 'container-images.yml'),
    'utf8',
  );
  assert.match(workflow, /- "api\/plugins\/default-extensions\.lock\.json"/u);
  assert.match(workflow, /- "scripts\/shell\/package-default-extension\.sh"/u);
});

test('AC-BOOT-3 default extension checksum verification uses the portable check flag', (t) => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'default-extension-packager-'));
  t.after(() => fs.rmSync(tempRoot, { recursive: true, force: true }));
  const binDir = path.join(tempRoot, 'bin');
  const outputDir = path.join(tempRoot, 'output');
  fs.mkdirSync(binDir);

  const commands = {
    jq: `#!/bin/sh
case "$*" in
  *artifact_url*) printf '%s\\n' 'https://example.invalid/extension.1flowbasepkg' ;;
  *checksum*) printf '%s\\n' '${'0'.repeat(64)}' ;;
  *bundled_path*) printf '%s\\n' 'bootstrap/extension.1flowbasepkg' ;;
  *) printf '%s\\n' '{"artifact_url":"https://example.invalid/extension.1flowbasepkg"}' ;;
esac
`,
    curl: `#!/bin/sh
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output" ]; then
    shift
    printf 'extension' > "$1"
    exit 0
  fi
  shift
done
exit 1
`,
    sha256sum: `#!/bin/sh
set -eu
test "$#" -eq 1
test "$1" = "-c"
cat >/dev/null
`,
  };

  for (const [name, contents] of Object.entries(commands)) {
    const commandPath = path.join(binDir, name);
    fs.writeFileSync(commandPath, contents, { mode: 0o755 });
  }

  const result = childProcess.spawnSync(
    'sh',
    [
      path.join(repoRoot, 'scripts', 'shell', 'package-default-extension.sh'),
      path.join(repoRoot, 'api', 'plugins', 'default-extensions.lock.json'),
      'amd64',
      outputDir,
    ],
    {
      encoding: 'utf8',
      env: { ...process.env, PATH: `${binDir}:${process.env.PATH}` },
    },
  );

  assert.equal(result.status, 0, result.stderr);
  assert.equal(
    fs.readFileSync(path.join(outputDir, 'extension.1flowbasepkg'), 'utf8'),
    'extension',
  );
});
