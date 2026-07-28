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
      max_tokens: 4096,
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
    max_output_tokens: 4096,
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
      input: PROMPT,
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

function firstDifference(left, right, path = '$') {
  if (Object.is(left, right)) return null;
  if (Array.isArray(left) && Array.isArray(right)) {
    const length = Math.max(left.length, right.length);
    for (let index = 0; index < length; index += 1) {
      const difference = firstDifference(left[index], right[index], `${path}[${index}]`);
      if (difference) return difference;
    }
    return null;
  }
  if (left && right && typeof left === 'object' && typeof right === 'object') {
    const keys = [...new Set([...Object.keys(left), ...Object.keys(right)])].sort();
    for (const key of keys) {
      const difference = firstDifference(left[key], right[key], `${path}.${key}`);
      if (difference) return difference;
    }
    return null;
  }
  return { path, direct: left, gateway: right };
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
      const difference = firstDifference(
        direct.request.fidelity_fixture,
        gateway.request.fidelity_fixture,
      );
      throw new Error(
        `${vector.id} normalized direct/Gateway request mismatch: ${JSON.stringify(difference)}`,
      );
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
