const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

test('release orchestration assertions accept Windows checkouts without weakening release gates', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'native-workflow-crlf-'));
  const repo = path.resolve(__dirname, '../../../..');
  try {
    const workflows = path.join(root, '.github/workflows');
    fs.mkdirSync(workflows, { recursive: true });
    for (const name of fs.readdirSync(path.join(repo, '.github/workflows'))) {
      if (!name.endsWith('.yml')) continue;
      const text = fs.readFileSync(path.join(repo, '.github/workflows', name), 'utf8');
      fs.writeFileSync(path.join(workflows, name), text.replace(/\r?\n/g, '\r\n'));
    }
    const entry = path.join(root, 'scripts/node/native-release/_tests/workflow.test.js');
    fs.mkdirSync(path.dirname(entry), { recursive: true });
    fs.copyFileSync(path.join(__dirname, 'workflow.test.js'), entry);
    const env = { ...process.env };
    delete env.NODE_TEST_CONTEXT;
    const result = spawnSync(process.execPath, ['--test', entry], { encoding: 'utf8', env, timeout: 30_000 });
    assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
