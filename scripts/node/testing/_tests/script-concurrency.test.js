const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { loadVerifyRuntimeConfig, LOCAL_VERIFY_CONFIG_FILE } = require('../verify-runtime.js');

test("script test concurrency reserves half the local CPUs and validates explicit overrides", (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'script-concurrency-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  for (const [availableParallelism, expected] of [[1, 1], [4, 2], [8, 4], [20, 10]]) {
    assert.equal(loadVerifyRuntimeConfig({ repoRoot, env: {}, availableParallelism }).scripts.testConcurrency, expected);
  }
  const configPath = path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE);
  fs.writeFileSync(configPath, JSON.stringify({ scripts: { testConcurrency: 1 } }));
  assert.equal(loadVerifyRuntimeConfig({ repoRoot, env: {}, availableParallelism: 4 }).scripts.testConcurrency, 1);
  for (const scripts of [[], { unknown: 1 }, { testConcurrency: 0 }, { testConcurrency: 1.5 }, { testConcurrency: 5 }]) {
    fs.writeFileSync(configPath, JSON.stringify({ scripts }));
    assert.throws(() => loadVerifyRuntimeConfig({ repoRoot, env: {}, availableParallelism: 4 }), /scripts/);
  }
  assert.equal(loadVerifyRuntimeConfig({ repoRoot, env: { CI: 'true' }, availableParallelism: 4 }).scripts.testConcurrency, 4);
});
