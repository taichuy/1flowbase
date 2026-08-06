const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
const manifestPath = path.join(
  repoRoot,
  'api',
  'plugins',
  'capability-plugins',
  '1flowbase',
  'manifest.yaml',
);
const webAppRoot = path.join(repoRoot, 'web', 'app');

function readHostModuleLocks() {
  const manifest = fs.readFileSync(manifestPath, 'utf8');
  const lockPattern = /^\s+- source: "([^"]+)"\n\s+version: "([^"]+)"\n\s+exports: \[([^\]]+)\]\n\s+binding: host$/gmu;

  return [...manifest.matchAll(lockPattern)].map((match) => ({
    source: match[1],
    version: match[2],
    exports: match[3].split(',').map((exportName) => exportName.trim()),
  }));
}

function loadProductionModule(moduleSource) {
  const script = `
    const path = require('node:path');
    const moduleSource = process.argv[1];
    const webAppRoot = process.argv[2];
    const packagePath = require.resolve(moduleSource + '/package.json', { paths: [webAppRoot] });
    const packageRoot = path.dirname(packagePath);
    const moduleValue = require(require.resolve(moduleSource, { paths: [webAppRoot] }));
    const exportNames = Object.keys(moduleValue);
    if (moduleSource === 'react') exportNames.push('default');
    process.stdout.write(JSON.stringify({
      version: require(packagePath).version,
      exports: [...new Set(exportNames)].sort(),
      packageRoot,
    }));
  `;
  const output = execFileSync(
    process.execPath,
    ['-e', script, moduleSource, webAppRoot],
    {
      cwd: repoRoot,
      encoding: 'utf8',
      env: { ...process.env, NODE_ENV: 'production' },
    },
  );

  return JSON.parse(output);
}

test('AC-001 production host provides every built-in Catalog component contract', () => {
  const hostModuleLocks = readHostModuleLocks();
  assert.deepEqual(
    hostModuleLocks.map(({ source }) => source),
    ['react', 'antd'],
    'the built-in Catalog host module inventory changed',
  );

  for (const lock of hostModuleLocks) {
    const productionModule = loadProductionModule(lock.source);
    const providedExports = new Set(productionModule.exports);
    const missingExports = lock.exports.filter(
      (exportName) => !providedExports.has(exportName),
    );

    assert.ok(lock.version, `${lock.source} Catalog contract identity must not be empty`);
    assert.deepEqual(
      missingExports,
      [],
      `${lock.source} Catalog exports must exist in ${lock.source}@${productionModule.version}`,
    );
  }
});
