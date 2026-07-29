'use strict';

const crypto = require('node:crypto');

const { SUCCESS_TERMINAL, TRANSPORT } = require('../contracts');
const { TERMINAL_STATUSES, queryRun } = require('../local-client-acceptance/durable');
const { runUuidFromProtocolId } = require('../characterize/durable-evidence');
const {
  createSseParser,
  protocolEventType,
  protocolRunId,
} = require('../characterize/stream-parsers');

const SOURCES = Object.freeze([
  Object.freeze({
    id: 'anthropic_messages',
    transport: TRANSPORT.ANTHROPIC_SSE,
    endpoint: 'anthropic_messages_url',
  }),
  Object.freeze({
    id: 'openai_chat',
    transport: TRANSPORT.CHAT_COMPLETIONS_SSE,
    endpoint: 'chat_completions_url',
  }),
  Object.freeze({
    id: 'openai_responses',
    transport: TRANSPORT.RESPONSES_SSE,
    endpoint: 'responses_url',
  }),
]);

const PROVIDERS = Object.freeze([
  Object.freeze({
    id: 'anthropic',
    restores: Object.freeze(['anthropic_messages']),
  }),
  Object.freeze({
    id: 'openai',
    restores: Object.freeze(['openai_chat', 'openai_responses']),
  }),
  Object.freeze({
    id: 'openai_compatible',
    restores: Object.freeze(['openai_chat']),
  }),
]);

const PROTOCOL_CONTEXT_PROFILE_MATRIX = Object.freeze(PROVIDERS.flatMap((provider) => (
  SOURCES.map((source) => Object.freeze({
    id: `${source.id}-to-${provider.id}`,
    source_protocol: source.id,
    provider: provider.id,
    transport: source.transport,
    endpoint: source.endpoint,
    residual_restored: provider.restores.includes(source.id),
  }))
)));

function expectedUpstreamPath(row) {
  if (row.provider === 'anthropic') return '/v1/messages';
  if (row.provider === 'openai' && row.source_protocol !== 'openai_chat') {
    return '/v1/responses';
  }
  return '/v1/chat/completions';
}

function requestBody(sourceProtocol, model, typedSystem, rawCanary) {
  const residual = { fixture_profile_extension: { opaque: rawCanary } };
  const prompt = `Protocol Context Profile matrix probe ${crypto.randomUUID()}`;
  if (sourceProtocol === 'anthropic_messages') {
    return {
      model,
      max_tokens: 64,
      stream: true,
      system: typedSystem,
      messages: [{ role: 'user', content: prompt }],
      ...residual,
    };
  }
  if (sourceProtocol === 'openai_chat') {
    return {
      model,
      stream: true,
      stream_options: { include_usage: true },
      messages: [
        { role: 'system', content: typedSystem },
        { role: 'user', content: prompt },
      ],
      ...residual,
    };
  }
  return {
    model,
    stream: true,
    instructions: typedSystem,
    input: prompt,
    ...residual,
  };
}

function jsonSha256(value) {
  return crypto.createHash('sha256').update(JSON.stringify(value)).digest('hex');
}

function expectedUrlDigest(pathname, rawCanary, restored) {
  const normalized = restored
    ? `${pathname}?fixture_query=${encodeURIComponent(rawCanary)}`
    : pathname;
  return jsonSha256(normalized);
}

function assertTypedSystem(row, arrival) {
  const body = arrival.request?.body;
  if (row.provider === 'anthropic') {
    if (!body?.keys?.includes('system')) {
      throw new Error(`${row.id} dropped Typed Native system before Anthropic wire`);
    }
    return;
  }
  if (expectedUpstreamPath(row) === '/v1/responses') {
    if (!body?.keys?.includes('instructions')) {
      throw new Error(`${row.id} dropped Typed Native system before Responses instructions`);
    }
    return;
  }
  if (!Number.isInteger(body?.messageCount) || body.messageCount < 2) {
    throw new Error(`${row.id} dropped Typed Native system before Chat messages`);
  }
}

function assertProfileProjection(row, arrival, rawCanary, upstreamModel) {
  const expectedPath = expectedUpstreamPath(row);
  if (arrival.request?.path !== expectedPath) {
    throw new Error(`${row.id} reached ${arrival.request?.path || 'unknown'} instead of ${expectedPath}`);
  }
  if (arrival.request?.body?.model !== upstreamModel) {
    throw new Error(`${row.id} did not use the Provider-owned upstream model`);
  }
  assertTypedSystem(row, arrival);
  const bodyHasResidual = arrival.request?.body?.keys?.includes('fixture_profile_extension') === true;
  const headerHasResidual = Object.hasOwn(
    arrival.request?.fidelity_fixture?.header_sha256 ?? {},
    'x-fixture-profile',
  );
  const actualUrlDigest = arrival.request?.fidelity_fixture?.url_sha256;
  const expectedDigest = expectedUrlDigest(expectedPath, rawCanary, row.residual_restored);
  if (bodyHasResidual !== row.residual_restored || headerHasResidual !== row.residual_restored) {
    throw new Error(`${row.id} residual body/header projection did not match the declared Profile`);
  }
  if (actualUrlDigest !== expectedDigest) {
    throw new Error(`${row.id} residual query projection did not match the declared Profile`);
  }
}

async function sendMatrixRequest(row, target, rawCanary, fetchImpl) {
  const endpoint = new URL(target.gateway[row.endpoint]);
  endpoint.searchParams.append('fixture_query', rawCanary);
  const typedSystem = `Typed Native system ${crypto.randomUUID()}`;
  const response = await fetchImpl(endpoint, {
    method: 'POST',
    headers: {
      accept: 'text/event-stream',
      authorization: `Bearer ${target.api_key}`,
      'content-type': 'application/json',
      'x-fixture-profile': rawCanary,
    },
    body: JSON.stringify(requestBody(
      row.source_protocol,
      target.model,
      typedSystem,
      rawCanary,
    )),
    signal: AbortSignal.timeout(60_000),
  });
  if (!response.ok) {
    const rawBody = await response.text();
    throw new Error(`${row.id} returned HTTP ${response.status}: ${rawBody.slice(0, 500)}`);
  }
  const protocolIds = [];
  const eventTypes = [];
  const parser = createSseParser((event) => {
    const type = protocolEventType(event);
    if (type) eventTypes.push(type);
    const id = protocolRunId(row.transport, event);
    if (id) protocolIds.push(id);
  });
  const reader = response.body.getReader();
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    parser.push(value);
  }
  parser.finish();
  const terminal = SUCCESS_TERMINAL[row.transport];
  if (eventTypes.filter((type) => type === terminal).length !== 1) {
    throw new Error(`${row.id} did not emit exactly one ${terminal}`);
  }
  const uniqueIds = [...new Set(protocolIds)];
  if (uniqueIds.length !== 1) throw new Error(`${row.id} did not expose one stable protocol id`);
  const runId = runUuidFromProtocolId(row.transport, uniqueIds[0]);
  if (!runId) throw new Error(`${row.id} protocol id did not contain a run UUID`);
  return runId;
}

async function waitForSucceededRun(target, runId, fetchImpl) {
  const deadline = Date.now() + 10_000;
  let run = null;
  do {
    run = await queryRun(target, runId, fetchImpl);
    if (TERMINAL_STATUSES.has(run.status)) break;
    if (Date.now() >= deadline) break;
    await new Promise((resolve) => setTimeout(resolve, 100));
  } while (true);
  if (run?.id !== runId || run.status !== 'succeeded') {
    throw new Error(`protocol context matrix durable run ${runId} remained ${run?.status || 'unknown'}`);
  }
  return run;
}

function arrivalsAfter(snapshot, sequence) {
  return snapshot.entries.filter(
    (entry) => entry.event === 'arrival' && entry.sequence > sequence,
  );
}

async function verifyProtocolContextProfileMatrix({ ready, mockSnapshot }, dependencies = {}) {
  const fetchImpl = dependencies.fetchImpl ?? globalThis.fetch;
  const rows = [];
  for (const row of PROTOCOL_CONTEXT_PROFILE_MATRIX) {
    const target = ready.targets[row.provider];
    const rawCanary = `profile-${crypto.randomBytes(18).toString('hex')}`;
    const before = mockSnapshot();
    const sequence = before.entries.at(-1)?.sequence ?? 0;
    const runId = await sendMatrixRequest(row, target, rawCanary, fetchImpl);
    const arrivals = arrivalsAfter(mockSnapshot(), sequence);
    if (arrivals.length !== 1) {
      throw new Error(`${row.id} produced ${arrivals.length} upstream arrivals instead of one`);
    }
    assertProfileProjection(row, arrivals[0], rawCanary, target.upstream_model);
    await waitForSucceededRun(target, runId, fetchImpl);
    rows.push({
      id: row.id,
      source_protocol: row.source_protocol,
      provider: row.provider,
      upstream_path: arrivals[0].request.path,
      residual_restored: row.residual_restored,
      durable_status: 'succeeded',
    });
  }
  return {
    schema_version: '1flowbase.protocol-context-profile-matrix/v1',
    verdict: 'PASS',
    rows,
  };
}

module.exports = {
  PROTOCOL_CONTEXT_PROFILE_MATRIX,
  assertProfileProjection,
  expectedUpstreamPath,
  verifyProtocolContextProfileMatrix,
};
