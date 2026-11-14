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

test('Gateway visibility macro keeps production implementations enforced and existing cfg exclusion semantics', () => {
  const relativePath = 'api/crates/api/public-gateway/src/probe.rs';
  const production = 'test_support_item! { pub(crate), [] fn run() { panic!("bad"); } }';
  assert.equal(scanRustSource({ relativePath, content: production })[0]?.rule, 'no-production-escape');
  assert.deepEqual(scanRustSource({ relativePath, content:
    'test_support_item! { , [#[cfg(test)]] fn helper() { panic!("test"); } }' }), []);
  // Unknown feature conditions could be production: the old scanner fails safe.
  assert.equal(scanRustSource({ relativePath, content:
    'test_support_item! { pub(super), [#[cfg(any(test, feature="test-support"))]] fn helper() { panic!("bad"); } }'
  })[0]?.rule, 'no-production-escape');
  assert.equal(scanRustSource({ relativePath, content:
    'test_support_item! { , [#[cfg(test)]] fn helper() { panic!("test"); } }\nfn real() { panic!("bad"); }'
  })[0]?.line, 2);
});


test('Plugins and Interaction production macros remain scanned while their real support directory is test-only', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'api-owner-support-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const owner of ['plugins-models', 'console-interaction']) {
    const source = `api/crates/api/${owner}/src/probe.rs`;
    const support = `api/crates/api/${owner}/src/_tests/support/probe.rs`;
    for (const file of [source, support]) {
      const target = path.join(root, file);
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.writeFileSync(target, 'test_support_item! { pub(crate), [] fn run() { panic!("bad"); } }');
    }
    assert.equal(scanRustSource({ relativePath: source, content:
      '#[cfg(any(test, feature="test-support"))] fn helper() { panic!("bad"); }'
    })[0]?.rule, 'no-production-escape');
  }
  const findings = collectRustBackendFindings({ repoRoot: root });
  assert.deepEqual(findings.map(({ file, rule }) => [file, rule]), [
    ['api/crates/api/console-interaction/src/probe.rs', 'no-production-escape'],
    ['api/crates/api/plugins-models/src/probe.rs', 'no-production-escape'],
  ]);
});
