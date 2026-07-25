'use strict';

const path = require('node:path');

const {
  claudeEnvironment,
  codexEnvironment,
  opencodeEnvironment,
} = require('../cli-smoke/environment');

function gatewayAuth(baseUrl, apiKey, protocol) {
  return {
    methodId: 'gateway',
    _meta: {
      gateway: {
        protocol,
        baseUrl,
        providerName: '1flowbase controlled gateway',
        headers: { authorization: `Bearer ${apiKey}` },
      },
    },
  };
}

function clientPlan(name, resolved, parentEnv, paths, gatewayBaseUrl, target) {
  const runtimeBin = path.dirname(resolved.adapter_executable);
  if (name === 'claude') {
    return {
      name,
      command: resolved.adapter_executable,
      args: [],
      cwd: paths.output,
      env: {
        ...claudeEnvironment(parentEnv, paths, gatewayBaseUrl, target.api_key),
        PATH: `${runtimeBin}:${parentEnv.PATH ?? ''}`,
        CLAUDE_CODE_EXECUTABLE: resolved.executable,
        ANTHROPIC_MODEL: target.model,
        ALLOW_BYPASS_PERMISSIONS: '1',
      },
      auth: gatewayAuth(gatewayBaseUrl, target.api_key, 'anthropic'),
      secrets: [target.api_key],
    };
  }
  if (name === 'codex') {
    return {
      name,
      command: resolved.adapter_executable,
      args: [],
      cwd: paths.output,
      env: {
        ...codexEnvironment(parentEnv, paths, target.api_key),
        PATH: `${runtimeBin}:${parentEnv.PATH ?? ''}`,
        CODEX_PATH: resolved.executable,
        CODEX_CONFIG: JSON.stringify({
          model: target.model,
          approval_policy: 'never',
          sandbox_mode: 'read-only',
          request_max_retries: 0,
          stream_max_retries: 0,
        }),
        INITIAL_AGENT_MODE: 'agent',
        NO_BROWSER: '1',
      },
      auth: gatewayAuth(`${gatewayBaseUrl}/v1`, target.api_key, 'openai'),
      secrets: [target.api_key],
    };
  }
  if (name === 'opencode') {
    return {
      name,
      command: resolved.adapter_executable,
      args: [...resolved.adapter_args, '--cwd', paths.output],
      cwd: paths.output,
      env: {
        ...opencodeEnvironment(parentEnv, paths, gatewayBaseUrl, target),
        PATH: `${runtimeBin}:${parentEnv.PATH ?? ''}`,
      },
      auth: null,
      secrets: [target.api_key],
    };
  }
  throw new Error(`unsupported ACP compatibility client: ${name}`);
}

module.exports = { clientPlan, gatewayAuth };
