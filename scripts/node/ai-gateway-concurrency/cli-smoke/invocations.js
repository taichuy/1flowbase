'use strict';

const fs = require('node:fs');
const path = require('node:path');

const TEXT_SENTINEL = '1flowbase gateway sentinel ok';
const TOOL_SENTINEL = '1flowbase gateway tool sentinel ok';
const TEXT_PROMPT = `Reply with exactly: ${TEXT_SENTINEL}`;

function promptForTurn(turn, paths) {
  if (turn === 'text') return TEXT_PROMPT;
  if (turn !== 'tool') throw new Error(`unsupported client turn: ${turn}`);
  return [
    '1flowbase-client-tool-vector',
    `TOOL_VECTOR_PATH=${path.join(paths.output, 'tool-vector.txt')}`,
    'Use the client-owned local read or shell tool requested by the provider.',
    `After its result is returned to the provider, print exactly: ${TOOL_SENTINEL}`,
  ].join(' ');
}

function tomlString(value) {
  return JSON.stringify(value);
}

function codexInvocation(executable, paths, gatewayBaseUrl, target, turn = 'text') {
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
      promptForTurn(turn, paths),
    ],
  };
}

function claudeInvocation(executable, paths, target, turn = 'text') {
  const settingsPath = path.join(paths.config, 'settings.json');
  fs.writeFileSync(settingsPath, '{}\n', { mode: 0o600 });
  return {
    executable,
    cwd: paths.output,
    settingsPath,
    args: [
      '--bare',
      '-p', promptForTurn(turn, paths),
      '--no-session-persistence',
      '--settings', settingsPath,
      '--output-format', 'stream-json',
      '--include-partial-messages',
      '--verbose',
      '--model', target.model,
      '--tools', turn === 'tool' ? 'Read' : '',
      '--disable-slash-commands',
      '--no-chrome',
    ],
  };
}

function opencodeInvocation(executable, paths, target, turn = 'text') {
  const adapter = path.join(__dirname, 'opencode-headless-client.js');
  return {
    executable: process.execPath,
    cwd: paths.output,
    clientSurface: 'headless-raw-event-stream',
    args: [
      adapter,
      '--opencode', executable,
      '--directory', paths.output,
      '--model', `oneflowbase_gateway/${target.model}`,
      '--prompt', promptForTurn(turn, paths),
    ],
  };
}

function sanitizedInvocation(invocation) {
  return {
    executable: invocation.executable,
    cwd: invocation.cwd,
    args: [...invocation.args],
    settings_path: invocation.settingsPath ?? null,
    client_surface: invocation.clientSurface ?? null,
    termination: invocation.terminateAfterSecondMarker ? 'ctrl-c-after-second-marker' : null,
  };
}

module.exports = {
  TEXT_PROMPT,
  TEXT_SENTINEL,
  TOOL_SENTINEL,
  claudeInvocation,
  codexInvocation,
  opencodeInvocation,
  promptForTurn,
  sanitizedInvocation,
};
