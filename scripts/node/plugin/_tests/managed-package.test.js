const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { createPluginPackage } = require('../package');

test('managed Windows artifacts include settings source and rewrite every matching binding entry', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'managed-package-'));
  try {
    const plugin = path.join(root, 'plugin');
    fs.mkdirSync(path.join(plugin, 'ui'), { recursive: true });
    fs.writeFileSync(path.join(plugin, 'manifest.yaml'), `manifest_version: 2
plugin_id: fixture
version: 0.1.0
vendor: Fixture
contract_version: 1flowbase.extension-bus/v1
runtime:
  protocol: stdio_json_multiplex_v1
  entry: bin/worker
managed:
  execution_bindings:
    - contribution_id: fixture.list
      runtime:
        protocol: stdio_json_multiplex_v1
        entry: bin/worker
    - contribution_id: fixture.other
      runtime:
        protocol: stdio_json
        entry: bin/other
`);
    fs.writeFileSync(path.join(plugin, 'ui/settings.tsx'), 'export default () => null;');
    fs.mkdirSync(path.join(plugin, 'src'));
    fs.writeFileSync(path.join(plugin, 'src/main.rs'), 'PRIVATE BUSINESS SOURCE');
    const binary = path.join(root, 'worker.exe');
    fs.writeFileSync(binary, 'fixture-binary');
    const artifact = createPluginPackage(plugin, path.join(root, 'out'), { runtimeBinaryFile: binary, targetTriple: 'x86_64-pc-windows-msvc' });
    const entries = spawnSync('tar', ['-tzf', artifact.packageFile], { encoding: 'utf8' });
    assert.equal(entries.status, 0);
    assert.match(entries.stdout, /ui\/settings.tsx/);
    assert.doesNotMatch(entries.stdout, /src\/main.rs/);
    const manifest = spawnSync('tar', ['-xOzf', artifact.packageFile, './manifest.yaml'], { encoding: 'utf8' });
    assert.equal(manifest.status, 0);
    assert.equal((manifest.stdout.match(/entry: bin\/worker.exe/g) || []).length, 2);
    assert.match(manifest.stdout, /entry: bin\/other\n/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
