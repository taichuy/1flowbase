const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { buildCiCommands, buildRepoCommands } = require('../../verify/index.js');
const {
  buildFrontendCommands,
  buildScriptTestCommand
} = require('../../test/index.js');
const { buildRuntimeGateCommand } = require('../../tooling/index.js');

function createNodeOverride() {
  const tempDir = fs.mkdtempSync(
    path.join(os.tmpdir(), 'oneflowbase-real-node-routing-')
  );
  const nodePath = path.join(
    tempDir,
    process.platform === 'win32' ? 'node.exe' : 'node'
  );

  fs.writeFileSync(nodePath, '#!/usr/bin/env bash\nexit 0\n', 'utf8');
  fs.chmodSync(nodePath, 0o755);

  return fs.realpathSync(nodePath);
}

test('repository and CI gates use the resolved real Node command for nested script commands', () => {
  const nodePath = createNodeOverride();
  const env = { PATH: '', ONEFLOWBASE_NODE: nodePath };
  const repoCommandPaths = buildRepoCommands({
    repoRoot: '/repo-root',
    env
  }).map((command) => command.command);
  const ciCommandPaths = buildCiCommands({ repoRoot: '/repo-root', env }).map(
    (command) => command.command
  );

  assert.equal(repoCommandPaths.length > 0, true);
  assert.equal(ciCommandPaths.length > 0, true);
  assert.deepEqual(new Set(repoCommandPaths), new Set([nodePath]));
  assert.deepEqual(new Set(ciCommandPaths), new Set([nodePath]));
});

test('frontend, script-test, and runtime gates use the resolved real Node command', () => {
  const nodePath = createNodeOverride();
  const env = { PATH: '', ONEFLOWBASE_NODE: nodePath };

  assert.equal(
    buildFrontendCommands({ layer: 'full', repoRoot: '/repo-root', env })[3]
      .command,
    nodePath
  );
  assert.equal(
    buildScriptTestCommand({
      repoRoot: '/repo-root',
      files: ['/repo-root/scripts/node/a.test.js'],
      env
    }).command,
    nodePath
  );
  assert.equal(
    buildRuntimeGateCommand({
      argv: ['snapshot', '/settings'],
      repoRoot: '/repo-root',
      env
    }).command,
    nodePath
  );
});
