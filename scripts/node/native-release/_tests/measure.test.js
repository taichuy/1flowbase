const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { resources } = require('../build/measure.js');

test('native resource receipts distinguish macOS bytes from Linux KiB', () => {
  assert.deepEqual(resources('12.00 real 8.50 user 1.25 sys\n 1048576 maximum resident set size', 'darwin'), { cpu_seconds: 9.75, max_rss_bytes: 1048576 });
  assert.deepEqual(resources('User time (seconds): 8.50\nSystem time (seconds): 1.25\nMaximum resident set size (kbytes): 1024', 'linux'), { cpu_seconds: 9.75, max_rss_bytes: 1048576 });
  assert.deepEqual(resources('', 'win32'), { cpu_seconds: null, max_rss_bytes: null });
});

test('the measured command preserves success and failure while writing evidence', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'native-measure-'));
  try {
    for (const code of [0, 7]) {
      const prefix = path.join(root, `phase-${code}`);
      const result = spawnSync(process.execPath, [path.resolve(__dirname, '../build/measure.js'), prefix, process.execPath, '-e', `console.error('fixture');process.exit(${code})`], { encoding: 'utf8', timeout: 30000 });
      assert.equal(result.status, code, result.stderr);
      const receipt = JSON.parse(fs.readFileSync(prefix + '.json'));
      assert.equal(receipt.exit_code, code);
      assert.ok(receipt.wall_seconds > 0);
      assert.match(fs.readFileSync(prefix + '.log', 'utf8'), /fixture/);
    }
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});
