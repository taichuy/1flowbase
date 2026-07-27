'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const pairedSource = require('../paired-source.lock.json');

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

test('AC-003/005/006/008/019/024: workflow delegates all blocking checks to one repository command', () => {
  assert.match(source, /Run the single blocking AI Gateway quality command/u);
  assert.match(source, /quality-gate\/cli\.js run/u);
  assert.equal(source.match(/quality-gate\/cli\.js run/gu)?.length, 1);
  assert.doesNotMatch(source, /cargo (test|build)|node --test/u);
  assert.doesNotMatch(source, /@openai\/codex|@anthropic-ai\/claude-code|opencode|npm install --global|@latest/u);
});

test('AC-027/028: gate uses paired provider source, empty credentials, and always uploads evidence', () => {
  assert.ok(source.includes(`ref: ${pairedSource.official_plugins.revision}`));
  assert.match(source, /OPENAI_API_KEY: ''/u);
  assert.match(source, /ANTHROPIC_API_KEY: ''/u);
  assert.match(source, /name: Upload bounded protocol evidence\n        if: always\(\)/u);
  assert.match(source, /tmp\/test-governance\/ai-gateway-quality-gate\/\*\*/u);
  assert.match(source, /tmp\/test-governance\/ai-gateway-concurrency\/\*\*/u);
  assert.match(source, /if-no-files-found: error/u);
});
