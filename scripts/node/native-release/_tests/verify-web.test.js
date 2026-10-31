const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { verifyWeb } = require('../verify-web.js');

test('shared web checksums accept GNU manifests with LF or CRLF and reject missing or changed assets', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'verify-web-'));
  try {
    fs.mkdirSync(path.join(root, 'dist/assets'), { recursive: true });
    const files = { 'index.html': '<script src="/assets/app.js"></script>', 'assets/app.js': 'console.log("shared")' };
    for (const [name, contents] of Object.entries(files)) fs.writeFileSync(path.join(root, 'dist', name), contents);
    const entries = Object.entries(files).map(([name, contents]) => `${createHash('sha256').update(contents).digest('hex')}  ./${name}`);
    const manifest = path.join(root, 'web-dist.sha256');
    for (const eol of ['\n', '\r\n']) {
      fs.writeFileSync(manifest, entries.join(eol) + eol);
      assert.equal(verifyWeb(root), 2);
    }
    fs.writeFileSync(path.join(root, 'dist/assets/app.js'), 'tampered');
    assert.throws(() => verifyWeb(root), /checksum mismatch/);
    fs.unlinkSync(path.join(root, 'dist/assets/app.js'));
    assert.throws(() => verifyWeb(root), /ENOENT/);
    fs.writeFileSync(manifest, '');
    assert.throws(() => verifyWeb(root), /Empty/);
    fs.writeFileSync(manifest, 'bad checksum\n');
    assert.throws(() => verifyWeb(root), /Invalid/);
    fs.writeFileSync(manifest, `${'a'.repeat(64)}  ./../outside\n`);
    assert.throws(() => verifyWeb(root), /escapes dist/);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});
