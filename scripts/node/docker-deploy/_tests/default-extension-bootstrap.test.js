const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
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
  assert.match(packager, /sha256sum --check --status/u);
});
