'use strict';

const path = require('node:path');

const ARTIFACT_SCHEMA = '1flowbase.local-client-acceptance/v1';
const TEXT_SENTINEL = '1flowbase gateway sentinel ok';
const TOOL_RESULT_SENTINEL = '1flowbase-client-tool-result';
const TOOL_FINAL_SENTINEL = '1flowbase gateway tool sentinel ok';
const TEXT_VECTOR = Object.freeze({
  id: 'text-canonical-sentinel',
  kind: 'text',
  prompt: `Reply with exactly: ${TEXT_SENTINEL}`,
  expected: Object.freeze({ final_marker: TEXT_SENTINEL, durable_runs: 1, provider_requests: 1 }),
});
const TOOL_VECTOR = Object.freeze({
  id: 'tool-two-turn',
  kind: 'tool',
  promptTemplate: [
    '1flowbase-client-tool-vector',
    'TOOL_VECTOR_PATH={{TOOL_PATH}}',
    'Use the client-owned local read or shell tool requested by the provider.',
    `After its result is returned to the provider, print exactly: ${TOOL_FINAL_SENTINEL}`,
  ].join(' '),
  expected: Object.freeze({
    final_marker: TOOL_FINAL_SENTINEL,
    durable_runs: 1,
    provider_requests: 2,
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

function targetFromProvider(provider, gatewayBaseUrl) {
  if (!provider?.application_id || !provider?.model || !provider?.api_key) {
    throw new Error('fixture provider target is incomplete');
  }
  return {
    applicationId: provider.application_id,
    model: provider.model,
    apiKey: provider.api_key,
    gatewayBaseUrl: (provider.gateway?.base_url || gatewayBaseUrl || '').replace(/\/$/u, ''),
    durable: provider.durable,
    runtimeActivity: provider.runtime_activity,
    activeStreams: provider.plugin_runner_active_streams,
  };
}

function targetsFromReady(ready) {
  if (ready?.schema_version !== '1flowbase.ai-gateway-fixture/v1') {
    throw new Error('Gateway fixture ready manifest schema mismatch');
  }
  const openai = targetFromProvider(ready.targets?.openai, ready.gateway_base_url);
  const anthropic = targetFromProvider(ready.targets?.anthropic, ready.gateway_base_url);
  return { claude: anthropic, opencode: openai, codex: openai };
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
      ...(websocket ? { RUST_LOG: 'codex_core::client=info' } : {}),
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
  TEXT_SENTINEL,
  TEXT_VECTOR,
  TOOL_FINAL_SENTINEL,
  TOOL_RESULT_SENTINEL,
  TOOL_VECTOR,
  buildClientPlan,
  promptFor,
  selectExecutionSurface,
  targetsFromReady,
};
