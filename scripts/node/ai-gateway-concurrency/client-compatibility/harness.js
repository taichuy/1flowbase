'use strict';

const { spawn } = require('node:child_process');

const { normalizedUpdate } = require('./normalize');
const { AcpRpcConnection } = require('./rpc');

const PROTOCOL_VERSION = 1;

function permissionResponse(params) {
  const options = Array.isArray(params?.options) ? params.options : [];
  const selected = options.find((option) => option.kind === 'allow_once') ?? options[0];
  if (!selected?.optionId) return { outcome: { outcome: 'cancelled' } };
  return { outcome: { outcome: 'selected', optionId: selected.optionId } };
}

function timelineRecorder(client) {
  const timeline = [];
  let sequence = 0;
  return {
    timeline,
    record(event, detail = {}) {
      timeline.push({ sequence: ++sequence, at_ms: Date.now(), client, event, ...detail });
    },
  };
}

async function runAcpClient(plan, options = {}) {
  const timeoutMs = options.timeoutMs ?? 120000;
  const prompts = options.prompts ?? [];
  if (prompts.length === 0) throw new Error('ACP compatibility run requires at least one prompt');
  const observation = timelineRecorder(plan.name);
  const child = spawn(plan.command, plan.args, {
    cwd: plan.cwd,
    env: plan.env,
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  const text = [];
  const tools = [];
  const releasedMarkers = new Set();
  const connection = new AcpRpcConnection(child, {
    timeoutMs,
    secrets: plan.secrets ?? [],
    record: (event, message) => observation.record(event, { message }),
    onRequest: async (method, params) => {
      if (method !== 'session/request_permission') return null;
      const response = permissionResponse(params);
      observation.record('permission_response', {
        tool_call_id: params?.toolCall?.toolCallId ?? null,
        outcome: response.outcome.outcome,
      });
      return response;
    },
    onNotification: async (method, params) => {
      if (method !== 'session/update') {
        observation.record('notification', { method });
        return;
      }
      const update = normalizedUpdate(params);
      observation.record(update.kind, { update });
      if (update.kind === 'text_delta') {
        text.push(update.text);
        for (const marker of options.releaseOnMarkers ?? []) {
          if (!releasedMarkers.has(marker) && text.join('').includes(marker)) {
            releasedMarkers.add(marker);
            observation.record('barrier_release_start', { marker });
            await options.onMarker?.(marker);
            observation.record('barrier_released', { marker });
          }
        }
      }
      if (update.kind === 'tool_call' || update.kind === 'tool_call_update') tools.push(update);
    },
  });

  try {
    const initialize = await connection.request('initialize', {
      protocolVersion: PROTOCOL_VERSION,
      clientCapabilities: {
        fs: { readTextFile: false, writeTextFile: false },
        terminal: false,
        auth: { terminal: false, _meta: { gateway: true } },
      },
      clientInfo: { name: '1flowbase-gateway-acceptance', title: '1flowbase Gateway Acceptance', version: '1' },
    });
    observation.record('initialized', {
      protocol_version: initialize.protocolVersion ?? null,
      agent_name: initialize.agentInfo?.name ?? null,
    });
    if (plan.auth) {
      const methods = Array.isArray(initialize.authMethods) ? initialize.authMethods.map((method) => method.id) : [];
      if (!methods.includes(plan.auth.methodId)) throw new Error(`${plan.name} ACP agent does not advertise gateway auth`);
      await connection.request('authenticate', plan.auth);
      observation.record('authenticated', { method: plan.auth.methodId });
    }
    const session = await connection.request('session/new', { cwd: plan.cwd, mcpServers: [] });
    if (typeof session.sessionId !== 'string' || session.sessionId.length === 0) {
      throw new Error(`${plan.name} ACP session/new omitted sessionId`);
    }
    observation.record('session_created', { session_id: session.sessionId });
    const turns = [];
    for (let index = 0; index < prompts.length; index += 1) {
      const textStart = text.length;
      const toolStart = tools.length;
      observation.record('prompt_started', { prompt_index: index });
      const response = await connection.request('session/prompt', {
        sessionId: session.sessionId,
        prompt: [{ type: 'text', text: prompts[index] }],
      });
      if (typeof response.stopReason !== 'string') throw new Error(`${plan.name} ACP prompt omitted stopReason`);
      observation.record('prompt_terminal', { prompt_index: index, stop_reason: response.stopReason });
      turns.push({
        prompt_index: index,
        stop_reason: response.stopReason,
        text: text.slice(textStart).join(''),
        tools: tools.slice(toolStart),
      });
    }
    return {
      schema_version: '1flowbase.acp-client-compatibility/v1',
      client: plan.name,
      session_id: session.sessionId,
      text: text.join(''),
      tools,
      turns,
      timeline: observation.timeline,
      stderr: connection.stderr,
    };
  } finally {
    await connection.close();
  }
}

module.exports = {
  PROTOCOL_VERSION,
  permissionResponse,
  runAcpClient,
};
