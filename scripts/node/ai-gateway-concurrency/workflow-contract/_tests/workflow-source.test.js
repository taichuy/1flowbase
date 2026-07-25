'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const GATE_PATH = path.resolve(__dirname, '../../../../../.github/workflows/ai-gateway-concurrency.yml');
const CANARY_PATH = path.resolve(__dirname, '../../../../../.github/workflows/ai-gateway-client-canary.yml');
const gate = fs.readFileSync(GATE_PATH, 'utf8');
const canary = fs.readFileSync(CANARY_PATH, 'utf8');
const localRunner = fs.readFileSync(path.resolve(__dirname, '../../local-acceptance/runner.js'), 'utf8');
const workflowRunner = fs.readFileSync(path.resolve(__dirname, '../runner.js'), 'utf8');

test('D7-AC-007/008: pinned compatibility lane blocks relevant PR and protected pushes', () => {
  const trigger = gate.slice(gate.indexOf('\non:'), gate.indexOf('\npermissions:'));
  assert.match(trigger, /pull_request:/u);
  assert.match(trigger, /push:[\s\S]*branches: \[dev\]/u);
  assert.match(trigger, /workflow_dispatch:/u);
  assert.doesNotMatch(trigger, /schedule:/u);
  assert.match(gate, /npm ci --prefix scripts\/node\/ai-gateway-concurrency\/client-compatibility\/runtime/u);
  assert.match(gate, /client-compatibility\.lock\.json/u);
  assert.doesNotMatch(gate, /npm install --global|@latest/u);
  assert.match(gate, /path: tmp\/test-governance\/\*\*/u);
});

test('D7-AC-008: version canary is scheduled, non-blocking, and cannot rewrite the pin', () => {
  assert.match(canary, /schedule:/u);
  assert.match(canary, /workflow_dispatch:/u);
  assert.match(canary, /continue-on-error: true/u);
  assert.match(canary, /client-compatibility\/canary\.js/u);
  assert.doesNotMatch(canary, /client-compatibility\.lock\.json.*>|git commit|git push/u);
  assert.doesNotMatch(gate, /schedule:/u);
});

test('D7-AC-007: workflow keeps credentials empty and always uploads bounded evidence', () => {
  assert.match(gate, /runs-on: ubuntu-24\.04/u);
  assert.match(gate, /node-version: 24/u);
  assert.match(gate, /OPENAI_API_KEY: ''/u);
  assert.match(gate, /ANTHROPIC_API_KEY: ''/u);
  assert.match(gate, /name: Upload bounded compatibility evidence\n        if: always\(\)/u);
  assert.match(gate, /if-no-files-found: error/u);
});

test('D7-AC-007: local and CI orchestration invoke the same portable compatibility command', () => {
  assert.match(localRunner, /runClientCompatibilityCommand/u);
  assert.match(workflowRunner, /runClientCompatibilityCommand/u);
  assert.match(localRunner, /client-compatibility\/runtime/u);
  assert.match(workflowRunner, /client-compatibility\/runtime/u);
});
