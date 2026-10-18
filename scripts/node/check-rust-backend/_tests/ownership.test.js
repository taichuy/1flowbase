const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { collectRustBackendFindings, scanRustSource } = require('../core.js');

const content = 'pub fn crash() { panic!("invalid"); }';

test('vendored upstream source is excluded while first-party source remains enforced', (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'rust-source-owner-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  for (const relativePath of [
    'api/vendor/tungstenite/examples/server.rs',
    'api/vendor/tungstenite/src/lib.rs',
    'api/crates/protocol/src/server.rs',
  ]) {
    const file = path.join(repoRoot, relativePath);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, content);
  }
  const findings = collectRustBackendFindings({ repoRoot });
  assert.deepEqual(findings.map(({ file, rule, severity }) => ({ file, rule, severity })), [{
    file: 'api/crates/protocol/src/server.rs', rule: 'no-production-escape', severity: 'error',
  }]);
});

test('vendor exclusion uses the exact owner directory rather than a name substring', () => {
  for (const relativePath of [
    'api/crates/vendor_adapter/src/lib.rs',
    'api/vendor-owned/src/lib.rs',
    'api/apps/api-server/src/vendor.rs',
  ]) {
    assert.equal(scanRustSource({ relativePath, content })[0]?.rule, 'no-production-escape');
  }
});
