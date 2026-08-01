'use strict';

const COUNT_TOKENS_METHODS = Object.freeze([
  'upstream_api',
  'model_tokenizer',
  'provider_estimate',
  'generic_estimate',
  'fallback_zero',
]);
const OFFICIAL_PROVIDERS = Object.freeze([
  'openai', 'anthropic', 'aliyun_bailian', 'deepseek', 'gemini', 'openai_compatible',
]);

const COUNT_TOKENS_GATE_MATRIX = Object.freeze([
  Object.freeze({ id: 'complete_envelope', owner: 'plugin-framework', filter: 'count_tokens', expected: 'typed_total' }),
  Object.freeze({ id: 'capability_boundary', owner: 'plugin-runner', filter: 'd1_p03_count_tokens_missing_capability', expected: 'generic_estimate' }),
  Object.freeze({ id: 'plugin_unavailable', owner: 'plugin-runner', filter: 'd1_p03_count_tokens_missing_plugin', expected: 'generic_estimate' }),
  Object.freeze({ id: 'upstream_success', owner: 'provider-conformance', filter: 'six_actual_packages', expected: 'upstream_api_or_provider_estimate' }),
  Object.freeze({ id: 'upstream_4xx', owner: 'api-server', filter: 'count_tokens', expected: 'typed_4xx' }),
  Object.freeze({ id: 'malformed_provider_result', owner: 'plugin-runner', filter: 'count_tokens_preserves_upstream_success_and_estimates_provider_failures', expected: 'generic_estimate' }),
  Object.freeze({ id: 'model_tokenizer_shape', owner: 'plugin-framework', filter: 'count_tokens', expected: 'model_tokenizer' }),
  Object.freeze({ id: 'unknown_media', owner: 'provider-conformance', filter: 'six_actual_packages', expected: 'partial_with_unknown_count' }),
  Object.freeze({ id: 'generic_estimate', owner: 'plugin-framework', filter: 'provider_count_tokens_estimator', expected: 'generic_estimate' }),
  Object.freeze({ id: 'fallback_zero', owner: 'plugin-framework', filter: 'count_tokens', expected: 'fallback_zero' }),
  Object.freeze({ id: 'anthropic_protocol_shape', owner: 'api-server', filter: 'count_tokens', expected: 'anthropic_input_tokens_only' }),
  Object.freeze({ id: 'generate_stream_regression', owner: 'workflow-contract', filter: 'blocking_transport_matrix', expected: 'unchanged' }),
]);

function countTokensOracleInventory(matrix = COUNT_TOKENS_GATE_MATRIX) {
  const ids = new Set(matrix.map((row) => row.id));
  const required = [
    'complete_envelope', 'capability_boundary', 'plugin_unavailable', 'upstream_success',
    'upstream_4xx', 'malformed_provider_result', 'model_tokenizer_shape', 'unknown_media',
    'generic_estimate', 'fallback_zero', 'anthropic_protocol_shape', 'generate_stream_regression',
  ];
  for (const id of required) {
    if (!ids.has(id)) throw new Error(`CountTokens quality gate omitted ${id}`);
  }
  if (new Set(COUNT_TOKENS_METHODS).size !== 5) {
    throw new Error('CountTokens method inventory must contain all five typed methods');
  }
  return {
    rows: matrix.length,
    methods: COUNT_TOKENS_METHODS,
    official_providers: OFFICIAL_PROVIDERS,
    matrix,
  };
}

module.exports = {
  COUNT_TOKENS_GATE_MATRIX,
  COUNT_TOKENS_METHODS,
  OFFICIAL_PROVIDERS,
  countTokensOracleInventory,
};
