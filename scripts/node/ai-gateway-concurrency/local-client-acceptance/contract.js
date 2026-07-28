'use strict';

const path = require('node:path');
const {
  CLAUDE_PROTOCOL_PROFILE,
  CLAUDE_PROTOCOL_VECTOR,
  CONTINUITY_VECTOR,
  LONG_TEXT_VECTOR,
  MEANINGFUL_GIT_VECTOR,
  PARALLEL_TOOL_VECTOR,
  PROVIDER_ERROR_VECTOR,
  SEQUENTIAL_TOOL_VECTOR,
  TEXT_SENTINEL,
  TEXT_VECTOR,
  TOOL_ASSETS,
  TOOL_FINAL_SENTINEL,
  TOOL_RESULT_SENTINEL,
  TOOL_VECTOR,
  VECTOR_MANIFEST,
  vectorsFor,
} = require('./vector-manifest');

const ARTIFACT_SCHEMA = '1flowbase.local-client-acceptance/v1';

const CLIENT_PROTOCOLS = Object.freeze({
  claude: Object.freeze(['anthropic_sse']),
  opencode: Object.freeze(['openai_chat_sse']),
  codex: Object.freeze(['responses_sse', 'responses_websocket']),
});

function promptTemplate(vector, turnIndex) {
  if (Array.isArray(vector.turns)) {
    const turn = vector.turns[turnIndex];
    if (!turn?.prompt) throw new Error(`vector ${vector.id} omitted turn ${turnIndex + 1}`);
    return turn.prompt;
  }
  if (vector.kind === 'text' && vector.prompt) return vector.prompt;
  if (vector.promptTemplate) return vector.promptTemplate;
  throw new Error(`vector ${vector.id || '<unknown>'} omitted its prompt`);
}

function promptReplacements(paths) {
  if (typeof paths === 'string') return { TOOL_PATH: paths };
  return {
    TOOL_PATH: paths?.toolFile,
    ...(paths?.toolAssets || {}),
    GIT_REPO_PATH: paths?.gitRepo,
  };
}

function promptFor(vector, paths, turnIndex = 0) {
  let prompt = promptTemplate(vector, turnIndex);
  const replacements = promptReplacements(paths);
  for (const placeholder of prompt.matchAll(/\{\{([A-Z0-9_]+)\}\}/gu)) {
    const value = replacements[placeholder[1]];
    if (!value || !path.isAbsolute(value)) {
      throw new Error(`vector ${vector.id} requires absolute ${placeholder[1]}`);
    }
    prompt = prompt.replaceAll(placeholder[0], value);
  }
  if (/\{\{[A-Z0-9_]+\}\}/u.test(prompt)) {
    throw new Error(`vector ${vector.id} retained an unresolved path placeholder`);
  }
  return prompt;
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
  const openaiCompatible = targetFromProvider(
    ready.targets?.openai_compatible,
    ready.gateway_base_url
  );
  return { claude: anthropic, opencode: openaiCompatible, codex: openai };
}

function codexProviderArguments(target, provider, websocket) {
  return [
    '-c', `model_provider=${JSON.stringify(provider)}`,
    '-c', `model_providers.${provider}.name=${JSON.stringify('1flowbase local acceptance')}`,
    '-c', `model_providers.${provider}.base_url=${JSON.stringify(`${target.gatewayBaseUrl}/v1`)}`,
    '-c', `model_providers.${provider}.env_key=${JSON.stringify('ONEFLOWBASE_APPLICATION_API_KEY')}`,
    '-c', `model_providers.${provider}.wire_api=${JSON.stringify('responses')}`,
    '-c', `model_providers.${provider}.requires_openai_auth=false`,
    '-c', `model_providers.${provider}.supports_websockets=${websocket}`,
    '-c', `model_providers.${provider}.request_max_retries=0`,
    '-c', `model_providers.${provider}.stream_max_retries=0`,
  ];
}

function codexPlan(binary, target, paths, vector, protocol, execution) {
  if (!CLIENT_PROTOCOLS.codex.includes(protocol)) throw new Error(`unsupported Codex protocol: ${protocol}`);
  const provider = 'oneflowbase_local_acceptance';
  const websocket = protocol === 'responses_websocket';
  const persistent = vector.turns.length > 1;
  const args = [
    'exec',
    ...(!persistent ? ['--ephemeral'] : []),
    '--ignore-user-config', '--ignore-rules', '--skip-git-repo-check',
    '--json', '--sandbox', vector.id === MEANINGFUL_GIT_VECTOR.id ? 'workspace-write' : 'read-only', '--model', target.model,
    ...codexProviderArguments(target, provider, websocket),
  ];
  if (execution.turnIndex > 0) {
    if (!execution.sessionId) throw new Error('Codex continuation requires a thread id');
    args.push('resume', execution.sessionId);
  }
  args.push(promptFor(vector, paths, execution.turnIndex));
  return {
    invocation: {
      executable: binary,
      args,
      cwd: vector.id === MEANINGFUL_GIT_VECTOR.id ? paths.gitRepo : paths.output,
    },
    environment: {
      CODEX_HOME: paths.config,
      ONEFLOWBASE_APPLICATION_API_KEY: target.apiKey,
      ...(websocket ? { RUST_LOG: 'codex_core::client=info' } : {}),
    },
    configFiles: [],
    client_surface: persistent ? 'codex-exec-resume' : 'codex-exec-ephemeral',
  };
}

function claudePlan(binary, target, paths, vector, protocol, execution) {
  if (protocol !== 'anthropic_sse') throw new Error(`unsupported Claude protocol: ${protocol}`);
  const settingsPath = path.join(paths.config, 'settings.json');
  const profile = vector.protocol_profile ?? null;
  const persistent = vector.turns.length > 1;
  const args = ['--bare', '-p', promptFor(vector, paths, execution.turnIndex)];
  if (persistent) {
    if (!execution.sessionId) throw new Error('Claude continuation requires a session id');
    args.push(execution.turnIndex === 0 ? '--session-id' : '--resume', execution.sessionId);
  } else {
    args.push('--no-session-persistence');
  }
  args.push(
    '--settings', settingsPath, '--output-format', 'stream-json', '--include-partial-messages',
    '--verbose', '--model', profile?.model || target.model,
    ...(profile ? ['--effort', profile.effort] : []),
    '--tools', vector.id === MEANINGFUL_GIT_VECTOR.id ? 'Read,Edit,Bash' : vector.kind === 'tools' ? 'Read' : '',
    '--disable-slash-commands', '--no-chrome',
  );
  return {
    invocation: {
      executable: binary,
      args,
      cwd: vector.id === MEANINGFUL_GIT_VECTOR.id ? paths.gitRepo : paths.output,
    },
    environment: {
      CLAUDE_CONFIG_DIR: paths.config,
      ANTHROPIC_BASE_URL: target.gatewayBaseUrl,
      ANTHROPIC_API_KEY: target.apiKey,
      CLAUDE_CODE_OAUTH_TOKEN: '',
      ...(vector.kind === 'error' ? { CLAUDE_CODE_MAX_RETRIES: '0' } : {}),
      ...(profile?.environment || {}),
    },
    configFiles: [{ path: settingsPath, content: '{}\n' }],
    client_surface: persistent ? 'claude-print-resume' : 'claude-print',
    ...(profile ? { protocol_profile: profile.expected_evidence } : {}),
  };
}

function opencodeConfig(target, provider) {
  return {
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
}

function opencodePlan(binary, target, paths, vector, protocol, execution) {
  if (protocol !== 'openai_chat_sse') throw new Error(`unsupported OpenCode protocol: ${protocol}`);
  const provider = 'oneflowbase_local_acceptance';
  const model = `${provider}/${target.model}`;
  const runSession = vector.kind === 'conversation' || vector.kind === 'error';
  let invocation;
  if (runSession) {
    const args = ['run', '--format', 'json', '--model', model];
    if (execution.turnIndex === 0) args.push('--title', `1flowbase ${vector.id}`);
    else {
      if (!execution.sessionId) throw new Error('OpenCode continuation requires a session id');
      args.push('--session', execution.sessionId);
    }
    args.push(promptFor(vector, paths, execution.turnIndex));
    invocation = {
      executable: binary,
      args,
      cwd: vector.id === MEANINGFUL_GIT_VECTOR.id ? paths.gitRepo : paths.output,
    };
  } else {
    const adapter = path.resolve(__dirname, '../cli-smoke/opencode-headless-client.js');
    invocation = {
      executable: process.execPath,
      args: [
        adapter, '--opencode', binary, '--directory',
        vector.id === MEANINGFUL_GIT_VECTOR.id ? paths.gitRepo : paths.output,
        '--model', model, '--prompt', promptFor(vector, paths, execution.turnIndex),
      ],
      cwd: vector.id === MEANINGFUL_GIT_VECTOR.id ? paths.gitRepo : paths.output,
    };
  }
  return {
    invocation,
    environment: {
      OPENCODE_CONFIG_DIR: paths.config,
      OPENCODE_CONFIG_CONTENT: JSON.stringify(opencodeConfig(target, provider)),
    },
    configFiles: [],
    client_surface: runSession ? 'opencode-run-session' : 'opencode-headless-http-session',
  };
}

function buildClientPlan(client, binary, rawTarget, paths, vector, protocol, execution = {}) {
  const target = commonTarget(rawTarget);
  const builders = { codex: codexPlan, claude: claudePlan, opencode: opencodePlan };
  if (!builders[client]) throw new Error(`unsupported local client: ${client}`);
  const turnIndex = execution.turnIndex ?? 0;
  const plan = builders[client](binary, target, paths, vector, protocol, {
    turnIndex,
    sessionId: execution.sessionId ?? null,
  });
  return {
    ...plan,
    client,
    protocol,
    vector_id: vector.id,
    turn_index: turnIndex,
    secrets: [target.apiKey],
  };
}

module.exports = {
  ARTIFACT_SCHEMA,
  CLAUDE_PROTOCOL_PROFILE,
  CLAUDE_PROTOCOL_VECTOR,
  CLIENT_PROTOCOLS,
  CONTINUITY_VECTOR,
  LONG_TEXT_VECTOR,
  MEANINGFUL_GIT_VECTOR,
  PARALLEL_TOOL_VECTOR,
  PROVIDER_ERROR_VECTOR,
  SEQUENTIAL_TOOL_VECTOR,
  TEXT_SENTINEL,
  TEXT_VECTOR,
  TOOL_ASSETS,
  TOOL_FINAL_SENTINEL,
  TOOL_RESULT_SENTINEL,
  TOOL_VECTOR,
  VECTOR_MANIFEST,
  buildClientPlan,
  promptFor,
  selectExecutionSurface,
  targetsFromReady,
  vectorsFor,
};
