'use strict';

const fs = require('node:fs');
const path = require('node:path');

const FIXED_PROMPT = 'Do not call any tools. Reply with exactly: 1flowbase gateway sentinel ok';

function tomlString(value) {
  return JSON.stringify(value);
}

function codexInvocation(executable, paths, gatewayBaseUrl, target) {
  const provider = 'oneflowbase_gateway';
  return {
    executable,
    cwd: paths.output,
    args: [
      'exec',
      '--ephemeral',
      '--ignore-user-config',
      '--ignore-rules',
      '--skip-git-repo-check',
      '--json',
      '--sandbox', 'read-only',
      '--model', target.model,
      '-c', `model_provider=${tomlString(provider)}`,
      '-c', `model_providers.${provider}.name=${tomlString('1flowbase gateway sentinel')}`,
      '-c', `model_providers.${provider}.base_url=${tomlString(`${gatewayBaseUrl}/v1`)}`,
      '-c', `model_providers.${provider}.env_key=${tomlString('ONEFLOWBASE_APPLICATION_API_KEY')}`,
      '-c', `model_providers.${provider}.wire_api=${tomlString('responses')}`,
      '-c', `model_providers.${provider}.requires_openai_auth=false`,
      '-c', `model_providers.${provider}.supports_websockets=false`,
      '-c', `model_providers.${provider}.request_max_retries=0`,
      '-c', `model_providers.${provider}.stream_max_retries=0`,
      FIXED_PROMPT,
    ],
  };
}

function claudeInvocation(executable, paths, target) {
  const settingsPath = path.join(paths.config, 'settings.json');
  fs.writeFileSync(settingsPath, '{}\n', { mode: 0o600 });
  return {
    executable,
    cwd: paths.output,
    settingsPath,
    args: [
      '--bare',
      '-p', FIXED_PROMPT,
      '--no-session-persistence',
      '--settings', settingsPath,
      '--output-format', 'stream-json',
      '--include-partial-messages',
      '--verbose',
      '--model', target.model,
      '--tools', '',
      '--disable-slash-commands',
      '--no-chrome',
    ],
  };
}

function opencodeInvocation(executable, paths, target) {
  return {
    executable,
    cwd: paths.output,
    args: [
      'run',
      '--pure',
      '--format', 'default',
      '--dir', paths.output,
      '--model', `oneflowbase_gateway/${target.model}`,
      FIXED_PROMPT,
    ],
  };
}

function sanitizedInvocation(invocation) {
  return {
    executable: invocation.executable,
    cwd: invocation.cwd,
    args: [...invocation.args],
    settings_path: invocation.settingsPath ?? null,
  };
}

module.exports = {
  FIXED_PROMPT,
  claudeInvocation,
  codexInvocation,
  opencodeInvocation,
  sanitizedInvocation,
};
