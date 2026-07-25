'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const WORKFLOW_PATH = path.resolve(__dirname, '../../../../../.github/workflows/ai-gateway-concurrency.yml');
const source = fs.readFileSync(WORKFLOW_PATH, 'utf8');

test('AC-029: protocol conformance blocks relevant PRs and protected pushes', () => {
  const trigger = source.slice(source.indexOf('\non:'), source.indexOf('\npermissions:'));
  assert.match(trigger, /workflow_dispatch:/u);
  assert.match(trigger, /pull_request:/u);
  assert.match(trigger, /push:[\s\S]*branches: \[dev\]/u);
  assert.doesNotMatch(trigger, /schedule:/u);
  assert.match(source, /name: AI Gateway Protocol Conformance Gate/u);
});

test('AC-003/005/006/008/019/024: gate owns protocol and conversation evidence, not client releases', () => {
  assert.match(source, /Run protocol harness structural tests/u);
  assert.match(source, /Verify complete-conversation state machines/u);
  assert.match(source, /-p control-plane application_public_api/u);
  assert.match(source, /-p api-server application_public_api/u);
  assert.match(source, /Run real protocol characterize contract/u);
  assert.doesNotMatch(source, /@openai\/codex|@anthropic-ai\/claude-code|opencode|npm install --global|@latest/u);
});

test('AC-027/028: gate uses paired provider source, empty credentials, and always uploads evidence', () => {
  assert.match(source, /paired-source\.lock\.json/u);
  assert.match(source, /OPENAI_API_KEY: ''/u);
  assert.match(source, /ANTHROPIC_API_KEY: ''/u);
  assert.match(source, /name: Upload bounded protocol evidence\n        if: always\(\)/u);
  assert.match(source, /path: tmp\/test-governance\/\*\*/u);
  assert.match(source, /if-no-files-found: error/u);
});
