'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const WORKFLOW_PATH = path.resolve(__dirname, '../../../../../.github/workflows/ai-gateway-concurrency.yml');
const source = fs.readFileSync(WORKFLOW_PATH, 'utf8');

test('AC-001: workflow has only a required manual trigger and exact SHA verification', () => {
  const trigger = source.slice(source.indexOf('\non:'), source.indexOf('\npermissions:'));
  assert.match(trigger, /workflow_dispatch:/u);
  for (const forbidden of ['push:', 'pull_request:', 'schedule:', 'workflow_call:']) assert.doesNotMatch(trigger, new RegExp(forbidden, 'u'));
  for (const input of ['main_source_sha', 'official_source_sha', 'profile']) {
    assert.match(trigger, new RegExp(`${input}:[\\s\\S]*?required: true`, 'u'));
  }
  assert.match(trigger, /type: choice[\s\S]*options:[\s\S]*- characterize/u);
  assert.doesNotMatch(trigger, /- regression/u);
  assert.equal((source.match(/\^\[0-9a-f\]\{40\}\$/gu) ?? []).length, 2);
  assert.equal((source.match(/rev-parse HEAD/gu) ?? []).length, 2);
});

test('AC-001/006: workflow pins runner, clients, real builds, and keeps credentials empty', () => {
  assert.match(source, /runs-on: ubuntu-24\.04/u);
  assert.match(source, /timeout-minutes: 90/u);
  assert.match(source, /permissions:\n  contents: read/u);
  assert.match(source, /cancel-in-progress: true/u);
  assert.match(source, /node-version: 24/u);
  assert.match(source, /@openai\/codex@0\.144\.1/u);
  assert.match(source, /@anthropic-ai\/claude-code@2\.1\.212/u);
  assert.match(source, /for provider_code in openai anthropic/u);
  assert.match(source, /scripts\/node\/plugin\/cli\.js package/u);
  assert.match(source, /-p api-server -p plugin-runner/u);
  assert.match(source, /OPENAI_API_KEY: ''/u);
  assert.match(source, /ANTHROPIC_API_KEY: ''/u);
  assert.doesNotMatch(source, /API_DATABASE_POOL_MAX_CONNECTIONS/u);
});

test('AC-007/008: artifact, summary, and cleanup are always isolated from default gates', () => {
  assert.match(source, /name: Finalize job summary and owned cleanup\n        if: always\(\)/u);
  assert.match(source, /name: Upload bounded characterize evidence\n        if: always\(\)/u);
  assert.match(source, /path: tmp\/test-governance\/ai-gateway-concurrency\/\*\*/u);
  assert.match(source, /AI_GATEWAY_ARTIFACT_ROOT\/workflow-runner\.log/u);
  assert.match(source, /if-no-files-found: error/u);
  assert.doesNotMatch(source, /quality-gate|verify\.yml|schedule|pull_request/u);
});
