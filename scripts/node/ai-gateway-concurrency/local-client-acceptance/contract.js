'use strict';

const path = require('node:path');

const ARTIFACT_SCHEMA = '1flowbase.local-client-acceptance/v1';
const LONG_SEGMENT = '1flowbase-long-repeated-segment';
const LONG_REPEAT_COUNT = 64;
const LONG_RESPONSE = Array(LONG_REPEAT_COUNT).fill(LONG_SEGMENT).join(' ');
const TEXT_VECTOR = Object.freeze({
  id: 'text-long-repeated',
  kind: 'text',
  prompt: `Reply with exactly this repeated body and nothing else: ${LONG_RESPONSE}`,
  expected: Object.freeze({ segment: LONG_SEGMENT, repetitions: LONG_REPEAT_COUNT }),
});
const TOOL_VECTOR = Object.freeze({
  id: 'tool-two-turn',
  kind: 'tool',
  promptTemplate: [
    'Use your client-owned local file read tool exactly once on {{TOOL_PATH}}.',
    'Return the file content to the provider, then after the second provider turn reply exactly:',
    '1flowbase-tool-two-turn-complete',
  ].join(' '),
  expected: Object.freeze({
    final_marker: '1flowbase-tool-two-turn-complete',
    timeline: Object.freeze([
      'client_started', 'tool_call_observed', 'tool_result_observed',
      'second_turn_observed', 'final_marker_observed', 'client_exited',
    ]),
  }),
});

const CLIENT_PROTOCOLS = Object.freeze({
  claude: Object.freeze(['anthropic_sse']),
  opencode: Object.freeze(['openai_chat_sse']),
  codex: Object.freeze(['responses_sse', 'responses_websocket']),
});

function promptFor(vector, toolPath) {
  if (vector.kind === 'text') return vector.prompt;
  if (!toolPath || !path.isAbsolute(toolPath)) throw new Error('tool vector path must be absolute');
  return vector.promptTemplate.replace('{{TOOL_PATH}}', toolPath);
}

function selectExecutionSurface(requested, capabilities = {}) {
  if (!['auto', 'tmux', 'acp-headless'].includes(requested)) {
    throw new Error(`unsupported execution surface: ${requested}`);
  }
  if ((requested === 'auto' || requested === 'acp-headless') && capabilities.acpHeadless) {
    return { status: 'selected', surface: 'acp-headless', reason: 'available' };
  }
  if ((requested === 'auto' || requested === 'tmux') && capabilities.tmux) {
    return { status: 'selected', surface: 'tmux', reason: 'available' };
  }
  return {
    status: 'skipped',
    surface: null,
    reason: requested === 'acp-headless' ? 'acp_headless_unavailable' : 'tmux_unavailable',
  };
}

function commonTarget(target) {
  if (!target?.model) throw new Error('client target model is required');
  if (!target?.apiKey) throw new Error('client target API key is required');
  if (!target?.gatewayBaseUrl) throw new Error('gateway base URL is required');
  return {
    model: target.model,
    apiKey: target.apiKey,
    gatewayBaseUrl: target.gatewayBaseUrl.replace(/\/$/u, ''),
  };
}

function codexPlan(binary, target, paths, vector, protocol) {
  if (!CLIENT_PROTOCOLS.codex.includes(protocol)) throw new Error(`unsupported Codex protocol: ${protocol}`);
  const provider = 'oneflowbase_local_acceptance';
  const websocket = protocol === 'responses_websocket';
  const prompt = promptFor(vector, paths.toolFile);
  return {
    invocation: {
      executable: binary,
      args: [
        'exec', '--ephemeral', '--ignore-user-config', '--ignore-rules', '--skip-git-repo-check',
        '--json', '--sandbox', 'read-only', '--model', target.model,
        '-c', `model_provider=${JSON.stringify(provider)}`,
        '-c', `model_providers.${provider}.name=${JSON.stringify('1flowbase local acceptance')}`,
        '-c', `model_providers.${provider}.base_url=${JSON.stringify(`${target.gatewayBaseUrl}/v1`)}`,
        '-c', `model_providers.${provider}.env_key=${JSON.stringify('ONEFLOWBASE_APPLICATION_API_KEY')}`,
        '-c', `model_providers.${provider}.wire_api=${JSON.stringify('responses')}`,
        '-c', `model_providers.${provider}.requires_openai_auth=false`,
        '-c', `model_providers.${provider}.supports_websockets=${websocket}`,
        '-c', `model_providers.${provider}.request_max_retries=0`,
        '-c', `model_providers.${provider}.stream_max_retries=0`,
        prompt,
      ],
      cwd: paths.output,
    },
    environment: {
      CODEX_HOME: paths.config,
      ONEFLOWBASE_APPLICATION_API_KEY: target.apiKey,
    },
    configFiles: [],
  };
}

function claudePlan(binary, target, paths, vector, protocol) {
  if (protocol !== 'anthropic_sse') throw new Error(`unsupported Claude protocol: ${protocol}`);
  const settingsPath = path.join(paths.config, 'settings.json');
  return {
    invocation: {
      executable: binary,
      args: [
        '--bare', '-p', promptFor(vector, paths.toolFile), '--no-session-persistence',
        '--settings', settingsPath, '--output-format', 'stream-json', '--include-partial-messages',
        '--verbose', '--model', target.model, '--tools', vector.kind === 'tool' ? 'Read' : '',
        '--disable-slash-commands', '--no-chrome',
      ],
      cwd: paths.output,
    },
    environment: {
      CLAUDE_CONFIG_DIR: paths.config,
      ANTHROPIC_BASE_URL: target.gatewayBaseUrl,
      ANTHROPIC_API_KEY: target.apiKey,
      CLAUDE_CODE_OAUTH_TOKEN: '',
    },
    configFiles: [{ path: settingsPath, content: '{}\n' }],
  };
}

function opencodePlan(binary, target, paths, vector, protocol) {
  if (protocol !== 'openai_chat_sse') throw new Error(`unsupported OpenCode protocol: ${protocol}`);
  const provider = 'oneflowbase_local_acceptance';
  const adapter = path.resolve(__dirname, '../cli-smoke/opencode-headless-client.js');
  const config = {
    model: `${provider}/${target.model}`,
    small_model: `${provider}/${target.model}`,
    provider: {
      [provider]: {
        id: provider,
        name: '1flowbase local acceptance',
        env: [],
        npm: '@ai-sdk/openai-compatible',
        models: {
          [target.model]: {
            id: target.model,
            name: target.model,
            tool_call: true,
            limit: { context: 100000, output: 10000 },
          },
        },
        options: { apiKey: target.apiKey, baseURL: `${target.gatewayBaseUrl}/v1` },
      },
    },
  };
  return {
    invocation: {
      executable: process.execPath,
      args: [
        adapter, '--opencode', binary, '--directory', paths.output,
        '--model', `${provider}/${target.model}`, '--prompt', promptFor(vector, paths.toolFile),
      ],
      cwd: paths.output,
    },
    environment: {
      OPENCODE_CONFIG_DIR: paths.config,
      OPENCODE_CONFIG_CONTENT: JSON.stringify(config),
    },
    configFiles: [],
  };
}

function buildClientPlan(client, binary, rawTarget, paths, vector, protocol) {
  const target = commonTarget(rawTarget);
  const builders = { codex: codexPlan, claude: claudePlan, opencode: opencodePlan };
  if (!builders[client]) throw new Error(`unsupported local client: ${client}`);
  const plan = builders[client](binary, target, paths, vector, protocol);
  return { ...plan, client, protocol, vector_id: vector.id, secrets: [target.apiKey] };
}

module.exports = {
  ARTIFACT_SCHEMA,
  CLIENT_PROTOCOLS,
  LONG_REPEAT_COUNT,
  LONG_SEGMENT,
  TEXT_VECTOR,
  TOOL_VECTOR,
  buildClientPlan,
  promptFor,
  selectExecutionSurface,
};
