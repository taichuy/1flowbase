'use strict';

const { REQUEST_FIDELITY_VECTORS } = require('../protocol-oracle/request-fidelity');

const PROMPT = 'Root #1477 request fidelity probe';

function queryString(entries) {
  const value = new URLSearchParams(entries).toString();
  return value ? `?${value}` : '';
}

function requestPair(vector, ready, upstreamBaseUrl) {
  const residual = vector.request;
  if (vector.ingress === 'openai_chat') {
    const base = {
      model: ready.targets.openai_compatible.model,
      messages: [{ role: 'user', content: PROMPT }],
      stream: true,
      reasoning_effort: residual.body.reasoning_effort,
      service_tier: residual.body.service_tier,
      fixture_body_extension: residual.body.fixture_body_extension,
    };
    return {
      directUrl: `${upstreamBaseUrl}${vector.expected_upstream_path}${queryString(residual.query)}`,
      gatewayUrl: `${ready.targets.openai_compatible.gateway.chat_completions_url}${queryString(residual.query)}`,
      gatewayToken: ready.targets.openai_compatible.api_key,
      directHeaders: { accept: 'application/json', ...residual.headers },
      gatewayHeaders: residual.headers,
      directBody: { ...base, stream_options: { include_usage: true } },
      gatewayBody: base,
    };
  }
  if (vector.ingress === 'anthropic_messages') {
    const common = {
      model: ready.targets.anthropic.model,
      stream: true,
      max_tokens: 32,
      thinking: residual.body.thinking,
      output_config: residual.body.output_config,
      context_management: residual.body.context_management,
      fixture_body_extension: residual.body.fixture_body_extension,
    };
    return {
      directUrl: `${upstreamBaseUrl}${vector.expected_upstream_path}${queryString(residual.query)}`,
      gatewayUrl: `${ready.targets.anthropic.gateway.anthropic_messages_url}${queryString(residual.query)}`,
      gatewayToken: ready.targets.anthropic.api_key,
      directHeaders: {
        accept: 'application/json',
        'anthropic-version': '2023-06-01',
        ...residual.headers,
      },
      gatewayHeaders: residual.headers,
      directBody: {
        ...common,
        messages: [{ role: 'user', content: [{ type: 'text', text: PROMPT }] }],
      },
      gatewayBody: {
        ...common,
        messages: [{ role: 'user', content: PROMPT }],
      },
    };
  }
  const common = {
    model: ready.targets.openai.model,
    stream: true,
    reasoning: residual.body.reasoning,
    truncation: residual.body.truncation,
    fixture_body_extension: residual.body.fixture_body_extension,
  };
  return {
    directUrl: `${upstreamBaseUrl}${vector.expected_upstream_path}${queryString(residual.query)}`,
    gatewayUrl: `${ready.targets.openai.gateway.responses_url}${queryString(residual.query)}`,
    gatewayToken: ready.targets.openai.api_key,
    directHeaders: { accept: 'text/event-stream', ...residual.headers },
    gatewayHeaders: residual.headers,
    directBody: {
      ...common,
      input: [{ role: 'user', content: PROMPT }],
    },
    gatewayBody: { ...common, input: PROMPT },
  };
}

async function send(url, token, headers, body) {
  const response = await fetch(url, {
    method: 'POST',
    headers: {
      accept: 'text/event-stream',
      authorization: `Bearer ${token}`,
      'content-type': 'application/json',
      ...headers,
    },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(60_000),
  });
  const responseBody = await response.text();
  if (!response.ok) {
    throw new Error(`request fidelity ${new URL(url).pathname} returned HTTP ${response.status}: ${responseBody.slice(0, 500)}`);
  }
}

function arrivalsAfter(snapshot, sequence) {
  return snapshot.entries.filter(
    (entry) => entry.event === 'arrival' && entry.sequence > sequence
  );
}

async function verifyGatewayRequestFidelity({ ready, upstreamBaseUrl, mockSnapshot }) {
  const rows = [];
  for (const vector of REQUEST_FIDELITY_VECTORS) {
    const before = mockSnapshot();
    const pair = requestPair(vector, ready, upstreamBaseUrl);
    await send(
      pair.directUrl,
      'direct-provider-secret',
      pair.directHeaders,
      pair.directBody
    );
    await send(
      pair.gatewayUrl,
      pair.gatewayToken,
      pair.gatewayHeaders,
      pair.gatewayBody
    );
    const arrivals = arrivalsAfter(mockSnapshot(), before.entries.at(-1)?.sequence ?? 0);
    if (arrivals.length !== 2) {
      throw new Error(`${vector.id} produced ${arrivals.length} upstream arrivals instead of two`);
    }
    const [direct, gateway] = arrivals;
    if (direct.request.semantic_sha256 !== gateway.request.semantic_sha256) {
      throw new Error(`${vector.id} normalized direct/Gateway request mismatch`);
    }
    rows.push({
      id: vector.id,
      ingress: vector.ingress,
      direct_sha256: direct.request.semantic_sha256,
      gateway_sha256: gateway.request.semantic_sha256,
      upstream_path: direct.request.path,
    });
  }
  return {
    schema_version: '1flowbase.ai-gateway-live-request-fidelity/v1',
    verdict: 'PASS',
    rows,
  };
}

module.exports = { requestPair, verifyGatewayRequestFidelity };
