'use strict';

const { MOCK_ROUTE, TRANSPORT } = require('../contracts');

const RESPONSES_WEBSOCKET_BETA = 'responses_websockets=2026-02-06';

function requiredString(value, label) {
  if (typeof value !== 'string' || value.trim() === '') throw new Error(`${label} is required`);
  return value.trim();
}

function websocketUrl(responsesUrl) {
  const url = new URL(requiredString(responsesUrl, 'Gateway Responses URL'));
  if (!['http:', 'https:'].includes(url.protocol)) {
    throw new Error(`Gateway Responses URL must use HTTP(S), received ${url.protocol}`);
  }
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  return url.toString();
}

function createGatewayTarget(ready) {
  const provider = ready?.targets?.openai;
  if (!provider || typeof provider !== 'object') throw new Error('ready manifest omitted OpenAI target');
  const responsesUrl = requiredString(provider.gateway?.responses_url, 'Gateway Responses URL');
  const target = {
    evidence_role: 'gateway-support-target',
    transport: TRANSPORT.RESPONSES_WEBSOCKET,
    url: websocketUrl(responsesUrl),
    application_id: requiredString(provider.application_id, 'OpenAI application id'),
    provider_instance_id: requiredString(provider.provider_instance_id, 'OpenAI provider instance id'),
    model: requiredString(provider.model, 'OpenAI published model'),
    api_key: requiredString(provider.api_key, 'OpenAI Application API key'),
    connect_headers: {
      authorization: `Bearer ${requiredString(provider.api_key, 'OpenAI Application API key')}`,
      'openai-beta': RESPONSES_WEBSOCKET_BETA,
    },
    durable: provider.durable,
    controlled_upstream: ready.controlled_upstream ?? null,
  };
  for (const [name, endpoint] of Object.entries({
    query_run: target.durable?.query_run?.url_template,
    list_runs: target.durable?.list_runs?.url,
  })) requiredString(endpoint, `OpenAI durable ${name} endpoint`);
  return target;
}

function createDirectUpstreamProbe(websocketBaseUrl) {
  const base = new URL(requiredString(websocketBaseUrl, 'direct mock WebSocket base URL'));
  if (!['ws:', 'wss:'].includes(base.protocol)) throw new Error('direct mock probe must use WebSocket');
  return Object.freeze({
    evidence_role: 'upstream-probe-only',
    transport: TRANSPORT.RESPONSES_WEBSOCKET,
    url: new URL(MOCK_ROUTE.RESPONSES, base).toString(),
  });
}

function publicTarget(target) {
  if (target?.evidence_role !== 'gateway-support-target') {
    throw new Error('Gateway support evidence requires a Gateway target');
  }
  return {
    evidence_role: target.evidence_role,
    transport: target.transport,
    url: target.url,
    application_id: target.application_id,
    provider_instance_id: target.provider_instance_id,
    model: target.model,
    credential: '[REDACTED]',
    beta: target.connect_headers?.['openai-beta'],
  };
}

module.exports = {
  RESPONSES_WEBSOCKET_BETA,
  createDirectUpstreamProbe,
  createGatewayTarget,
  publicTarget,
  websocketUrl,
};
