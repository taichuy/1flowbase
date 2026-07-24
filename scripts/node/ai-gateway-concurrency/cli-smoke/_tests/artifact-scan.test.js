'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { assertNoArtifactSecrets } = require('../artifact-scan');

// D3-AC-008: the final gate scans bytes across every produced artifact family.
test('artifact scan reports the exact leaking file and accepts redacted trees', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'd3-artifact-scan-'));
  const canary = 'sk-controlled-artifact-canary';
  try {
    fs.mkdirSync(path.join(root, 'producer'));
    fs.writeFileSync(path.join(root, 'manifest.json'), '{"api_key":"<redacted>"}\n');
    fs.writeFileSync(path.join(root, 'producer', 'timeline.jsonl'), `${canary}\n`);
    assert.throws(() => assertNoArtifactSecrets([root], [canary]), /producer.*timeline\.jsonl/u);
    fs.writeFileSync(path.join(root, 'producer', 'timeline.jsonl'), '<redacted-application-key>\n');
    assert.equal(assertNoArtifactSecrets([root], [canary]).length, 2);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
