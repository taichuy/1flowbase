'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  COUNT_TOKENS_GATE_MATRIX,
  countTokensOracleInventory,
} = require('../count-tokens-matrix');

test('Root #1556 P12 freezes the finite CountTokens quality-gate acceptance matrix', () => {
  const inventory = countTokensOracleInventory();
  assert.equal(inventory.rows, 12);
  assert.deepEqual(inventory.methods, [
    'upstream_api', 'model_tokenizer', 'provider_estimate', 'generic_estimate', 'fallback_zero',
  ]);
  assert.deepEqual(inventory.official_providers, [
    'openai', 'anthropic', 'aliyun_bailian', 'deepseek', 'gemini', 'openai_compatible',
  ]);
});

test('Root #1556 P12 controlled negative rejects a missing fallback_zero row', () => {
  assert.throws(
    () => countTokensOracleInventory(
      COUNT_TOKENS_GATE_MATRIX.filter((row) => row.id !== 'fallback_zero'),
    ),
    /omitted fallback_zero/u,
  );
});
