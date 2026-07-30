#!/usr/bin/env node
'use strict';

const path = require('node:path');
const { TRANSPORT } = require('../ai-gateway-concurrency/contracts');
const {
  runDirectMockCharacterize,
  runGatewayCharacterize,
} = require('../ai-gateway-concurrency/characterize/engine');

function usage() {
  return `Usage:
  node scripts/node/cli/ai-gateway-concurrency.js --profile characterize --mode direct-mock [--repo-root <path>] [--timeout-ms <ms>]
  node scripts/node/cli/ai-gateway-concurrency.js --profile characterize --mode gateway \\
    --responses-sse-url <url> --mock-responses-websocket-url <url> --anthropic-sse-url <url> \\
    --openai-api-key-env <environment-variable> --anthropic-api-key-env <environment-variable> \\
    --openai-model <published-model> --anthropic-model <published-model> \\
    [--repo-root <path>] [--timeout-ms <ms>]

Writes report.md, summary.json, and events.jsonl below
tmp/test-governance/ai-gateway-concurrency/. Characterize applies correctness
contracts only; timing metrics are observations and have no absolute budget.
Gateway mode sends SSE to the public gateway URLs and WebSocket only to the
explicit deterministic Mock URL; it does not define a public WebSocket contract.
`;
}

function parseCliArgs(argv, env = process.env) {
  const values = new Map();
  const valueArgs = new Set([
    '--profile', '--mode', '--repo-root', '--timeout-ms', '--responses-sse-url',
    '--mock-responses-websocket-url', '--anthropic-sse-url',
    '--openai-api-key-env', '--anthropic-api-key-env',
    '--openai-model', '--anthropic-model',
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--help' || argument === '-h') {
      if (argv.length !== 1) throw new Error('--help cannot be combined with other arguments');
      return { help: true };
    }
    if (!valueArgs.has(argument) || !argv[index + 1] || argv[index + 1].startsWith('--')) {
      throw new Error(`invalid argument: ${argument}`);
    }
    if (values.has(argument)) throw new Error(`duplicate argument: ${argument}`);
    values.set(argument, argv[index + 1]);
    index += 1;
  }
  if (values.get('--profile') !== 'characterize') {
    throw new Error('--profile must be characterize; no regression budget is approved');
  }
  const mode = values.get('--mode');
  if (!['direct-mock', 'gateway'].includes(mode)) throw new Error('--mode must be direct-mock or gateway');
  const repoRoot = path.resolve(values.get('--repo-root') ?? path.join(__dirname, '..', '..', '..'));
  const timeoutText = values.get('--timeout-ms') ?? '5000';
  const timeoutMs = Number(timeoutText);
  if (!Number.isInteger(timeoutMs) || timeoutMs < 100 || timeoutMs > 60_000) {
    throw new Error('--timeout-ms must be an integer between 100 and 60000');
  }
  const gatewayArgs = [
    '--responses-sse-url',
    '--mock-responses-websocket-url',
    '--anthropic-sse-url',
    '--openai-api-key-env',
    '--anthropic-api-key-env',
    '--openai-model',
    '--anthropic-model',
  ];
  if (mode === 'direct-mock') {
    for (const argument of gatewayArgs) {
      if (values.has(argument)) throw new Error(`${argument} is only valid in gateway mode`);
    }
    return { help: false, mode, repoRoot, timeoutMs };
  }
  for (const argument of gatewayArgs) {
    if (!values.has(argument)) throw new Error(`missing required argument: ${argument}`);
  }
  const responsesApiKeyEnvironment = values.get('--openai-api-key-env');
  const anthropicApiKeyEnvironment = values.get('--anthropic-api-key-env');
  if (responsesApiKeyEnvironment === anthropicApiKeyEnvironment) {
    throw new Error('OpenAI and Anthropic API keys must use distinct environment variables');
  }
  const responsesApiKey = env[responsesApiKeyEnvironment];
  const anthropicApiKey = env[anthropicApiKeyEnvironment];
  if (!responsesApiKey?.trim()) throw new Error(`API key environment variable is empty: ${responsesApiKeyEnvironment}`);
  if (!anthropicApiKey?.trim()) throw new Error(`API key environment variable is empty: ${anthropicApiKeyEnvironment}`);
  if (responsesApiKey === anthropicApiKey) throw new Error('OpenAI and Anthropic Application API keys must be distinct');
  const openaiModel = values.get('--openai-model').trim();
  const anthropicModel = values.get('--anthropic-model').trim();
  if (!openaiModel) throw new Error('--openai-model must name a published model');
  if (!anthropicModel) throw new Error('--anthropic-model must name a published model');
  const responsesSseUrl = values.get('--responses-sse-url');
  const openaiGatewayOrigin = new URL(responsesSseUrl).origin;
  return {
    help: false,
    mode,
    repoRoot,
    timeoutMs,
    authorizationTokenByTransport: {
      [TRANSPORT.RESPONSES_SSE]: responsesApiKey,
      [TRANSPORT.CHAT_COMPLETIONS_SSE]: responsesApiKey,
      [TRANSPORT.ANTHROPIC_SSE]: anthropicApiKey,
    },
    modelByTransport: {
      [TRANSPORT.RESPONSES_SSE]: openaiModel,
      [TRANSPORT.CHAT_COMPLETIONS_SSE]: openaiModel,
      [TRANSPORT.ANTHROPIC_SSE]: anthropicModel,
    },
    endpointSet: {
      [TRANSPORT.RESPONSES_SSE]: responsesSseUrl,
      [TRANSPORT.RESPONSES_WEBSOCKET]: values.get('--mock-responses-websocket-url'),
      [TRANSPORT.CHAT_COMPLETIONS_SSE]: `${openaiGatewayOrigin}/v1/chat/completions`,
      [TRANSPORT.ANTHROPIC_SSE]: values.get('--anthropic-sse-url'),
    },
  };
}

async function main(argv = process.argv.slice(2), env = process.env) {
  const options = parseCliArgs(argv, env);
  if (options.help) {
    process.stdout.write(usage());
    return 0;
  }
  const result = options.mode === 'direct-mock'
    ? await runDirectMockCharacterize(options)
    : await runGatewayCharacterize(options);
  process.stdout.write(`[ai-gateway-concurrency] ${result.summary.verdict}: ${result.artifacts.outputDirectory}\n`);
  return result.summary.verdict === 'PASS' ? 0 : 1;
}

if (require.main === module) {
  main().then((status) => { process.exitCode = status; }).catch((error) => {
    process.stderr.write(`[ai-gateway-concurrency] ${error.message}\n`);
    process.exitCode = 1;
  });
}

module.exports = { main, parseCliArgs, usage };
