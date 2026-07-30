'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { randomUUID } = require('node:crypto');
const { spawnSync } = require('node:child_process');

const { narrowEnvironment } = require('./environment');
const { executeTmuxInvocation, parseJsonLines } = require('./runner');

const GIT_HISTORY_COMMAND = "git log -3 --format='%h %s'";
const CLAUDE_REPOSITORY_HISTORY_PROMPT = '查看最近三次代码提交';
const CLAUDE_REPOSITORY_FOLLOWUP_PROMPT =
  '不太理解，这个逻辑？什么情况下会触发这种透传协议？这个透传协议是根据我们选中变量选的吗';

function readRepositoryGitHistory(repository, spawnSyncImpl = spawnSync) {
  const result = spawnSyncImpl('git', ['log', '-3', '--format=%h%x09%s'], {
    cwd: repository,
    encoding: 'utf8',
    maxBuffer: 1024 * 1024,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`git history oracle exited with ${result.status}`);
  const commits = result.stdout.trimEnd().split('\n').filter(Boolean).map((line) => {
    const separator = line.indexOf('\t');
    if (separator <= 0) throw new Error('git history oracle emitted an invalid row');
    return { shortHash: line.slice(0, separator), subject: line.slice(separator + 1) };
  });
  if (commits.length !== 3) throw new Error(`git history oracle expected 3 commits, received ${commits.length}`);
  return commits;
}

function claudeRepositoryHistoryInvocation(
  executable,
  paths,
  model,
  { sessionId, turnIndex = 0 } = {},
) {
  if (!sessionId) throw new Error('Claude repository-history invocation requires a session id');
  fs.mkdirSync(paths.config, { recursive: true, mode: 0o700 });
  const settingsPath = path.join(paths.config, 'settings.json');
  fs.writeFileSync(settingsPath, '{}\n', { mode: 0o600 });
  return {
    executable,
    cwd: paths.repository,
    settingsPath,
    args: [
      '--bare',
      '-p', turnIndex === 0
        ? CLAUDE_REPOSITORY_HISTORY_PROMPT
        : CLAUDE_REPOSITORY_FOLLOWUP_PROMPT,
      turnIndex === 0 ? '--session-id' : '--resume', sessionId,
      '--settings', settingsPath,
      '--output-format', 'stream-json',
      '--include-partial-messages',
      '--verbose',
      '--model', model,
      '--tools', turnIndex === 0 ? 'Bash' : '',
      ...(turnIndex === 0 ? ['--allowedTools', 'Bash(git log -3 --format=*)'] : []),
      '--permission-mode', 'dontAsk',
      '--disable-slash-commands',
      '--no-chrome',
    ],
  };
}

function inspectClaudeFollowupResult(result) {
  let events = [];
  try {
    events = parseJsonLines(result.stdout.text, 'claude');
  } catch {
    events = [];
  }
  const terminal = events.findLast((event) => event.type === 'result');
  const answerText = typeof terminal?.result === 'string' ? terminal.result.trim() : '';
  return {
    ok: result.exit_code === 0
      && !result.timed_out
      && !result.stdout.overflow
      && !result.stderr.overflow
      && terminal?.is_error !== true
      && answerText.length > 0,
    answerPresent: answerText.length > 0,
    exitCode: result.exit_code,
    timedOut: result.timed_out,
  };
}

function claudeExternalMessageIds(result) {
  try {
    return parseJsonLines(result.stdout.text, 'claude').flatMap((event) => (
      event.type === 'stream_event'
        && event.event?.type === 'message_start'
        && typeof event.event.message?.id === 'string'
        ? [event.event.message.id]
        : []
    ));
  } catch {
    return [];
  }
}

function inspectClaudeRepositoryHistoryResult(result, expected, expectedWireModel = null) {
  let events = [];
  try {
    events = parseJsonLines(result.stdout.text, 'claude');
  } catch {
    events = [];
  }
  const toolUses = events.flatMap((event) => (
    event.type === 'assistant' && Array.isArray(event.message?.content)
      ? event.message.content.filter((block) => block?.type === 'tool_use')
      : []
  ));
  const bashToolUses = toolUses.filter((toolUse) => toolUse.name === 'Bash');
  const matchingToolUses = bashToolUses.filter((toolUse) => (
    typeof toolUse.input?.command === 'string'
      && toolUse.input.command.trim() === GIT_HISTORY_COMMAND
  ));
  const unexpectedBashToolCalls = bashToolUses.length - matchingToolUses.length;
  const completedToolUseIds = new Set(events.flatMap((event) => (
    event.type === 'user' && Array.isArray(event.message?.content)
      ? event.message.content
        .filter((block) => block?.type === 'tool_result' && typeof block.tool_use_id === 'string')
        .map((block) => block.tool_use_id)
      : []
  )));
  const completedGitToolCalls = matchingToolUses
    .filter((toolUse) => completedToolUseIds.has(toolUse.id)).length;
  const answerText = events
    .filter((event) => event.type === 'result' && typeof event.result === 'string')
    .map((event) => event.result)
    .at(-1) || '';
  const expectedAnswer = expected
    .map(({ shortHash, subject }) => `${shortHash} ${subject}`)
    .join('\n');
  let searchFrom = 0;
  let matchedCommitCount = 0;
  for (const { shortHash, subject } of expected) {
    const position = answerText.indexOf(`${shortHash} ${subject}`, searchFrom);
    if (position === -1) break;
    matchedCommitCount += 1;
    searchFrom = position + shortHash.length + subject.length + 1;
  }
  const exactOrder = expected.length === 3 && matchedCommitCount === expected.length;
  const exactAnswer = answerText.trim() === expectedAnswer;
  const observedWireModels = [...new Set(events.flatMap((event) => (
    event.type === 'stream_event'
      && event.event?.type === 'message_start'
      && typeof event.event.message?.model === 'string'
      ? [event.event.message.model]
      : []
  )))];
  const wireModelMatched = expectedWireModel === null
    || observedWireModels.includes(expectedWireModel);
  const ok = result.exit_code === 0
    && !result.timed_out
    && !result.stdout.overflow
    && !result.stderr.overflow
    && matchingToolUses.length === 1
    && unexpectedBashToolCalls === 0
    && completedGitToolCalls === 1
    && exactOrder
    && exactAnswer
    && wireModelMatched;
  return {
    ok,
    expectedCount: expected.length,
    matchingBashToolCalls: matchingToolUses.length,
    unexpectedBashToolCalls,
    completedGitToolCalls,
    matchedCommitCount,
    exactOrder,
    exactAnswer,
    observedWireModels,
    wireModelMatched,
    exitCode: result.exit_code,
    timedOut: result.timed_out,
  };
}

async function runClaudeRepositoryHistorySmoke(options, dependencies = {}) {
  const repository = path.resolve(options.repository);
  if (!fs.statSync(repository).isDirectory()) throw new Error('repository must be a directory');
  const model = String(options.model || '').trim();
  if (!model) throw new Error('model is required');
  const baseUrl = options.baseUrl || process.env.ANTHROPIC_BASE_URL;
  const authToken = options.authToken || process.env.ANTHROPIC_AUTH_TOKEN;
  const apiKey = options.apiKey || process.env.ANTHROPIC_API_KEY;
  if (!baseUrl) throw new Error('ANTHROPIC_BASE_URL is required');
  if (!authToken && !apiKey) throw new Error('Anthropic credential environment is required');
  const parsedBaseUrl = new URL(baseUrl);
  if (parsedBaseUrl.username || parsedBaseUrl.password) throw new Error('Anthropic base URL must not contain credentials');
  const proxyUrl = options.proxyUrl ? new URL(options.proxyUrl) : null;
  if (proxyUrl && !['http:', 'https:'].includes(proxyUrl.protocol)) {
    throw new Error('proxy URL must use http or https');
  }
  if (proxyUrl?.username || proxyUrl?.password) throw new Error('proxy URL must not contain credentials');

  const outputRoot = path.resolve(options.evidenceRoot || path.join(
    'tmp', 'test-governance', 'claude-repository-history'
  ));
  fs.mkdirSync(outputRoot, { recursive: true, mode: 0o700 });
  const paths = {
    repository,
    config: path.join(outputRoot, 'config'),
    home: path.join(outputRoot, 'home'),
  };
  fs.mkdirSync(paths.home, { recursive: true, mode: 0o700 });
  const executable = options.claudeExecutable || process.env.CLAUDE_CODE_EXECUTABLE || 'claude';
  const sessionId = options.sessionId || randomUUID();
  const environment = narrowEnvironment(dependencies.parentEnv || process.env, paths.home);
  environment.CLAUDE_CONFIG_DIR = paths.config;
  environment.ANTHROPIC_BASE_URL = parsedBaseUrl.toString().replace(/\/$/u, '');
  environment.CLAUDE_CODE_MAX_RETRIES = '0';
  environment.CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC = '1';
  if (proxyUrl) {
    const normalizedProxyUrl = proxyUrl.toString().replace(/\/$/u, '');
    environment.HTTP_PROXY = normalizedProxyUrl;
    environment.HTTPS_PROXY = normalizedProxyUrl;
    environment.http_proxy = normalizedProxyUrl;
    environment.https_proxy = normalizedProxyUrl;
  }
  if (authToken) environment.ANTHROPIC_AUTH_TOKEN = authToken;
  if (!authToken && apiKey) environment.ANTHROPIC_API_KEY = apiKey;
  const secret = authToken || apiKey;
  const expected = readRepositoryGitHistory(repository, dependencies.spawnSyncImpl || spawnSync);
  const execute = dependencies.executeTmuxInvocation || executeTmuxInvocation;
  const firstResult = await execute(
    claudeRepositoryHistoryInvocation(executable, paths, model, { sessionId, turnIndex: 0 }),
    environment,
    {
      artifactDirectory: path.join(outputRoot, 'tmux-turn-1'),
      timeoutMs: options.timeoutMs || 180000,
      secrets: [secret],
    }
  );
  const wireModel = model.replace(/\[1m\]$/iu, '');
  const firstTurn = inspectClaudeRepositoryHistoryResult(firstResult, expected, wireModel);
  let secondResult = null;
  let secondTurn = { ok: false, answerPresent: false, exitCode: null, timedOut: false };
  if (firstTurn.ok) {
    secondResult = await execute(
      claudeRepositoryHistoryInvocation(executable, paths, model, { sessionId, turnIndex: 1 }),
      environment,
      {
        artifactDirectory: path.join(outputRoot, 'tmux-turn-2'),
        timeoutMs: options.timeoutMs || 180000,
        secrets: [secret],
      }
    );
    secondTurn = inspectClaudeFollowupResult(secondResult);
  }
  const messageIds = [
    ...claudeExternalMessageIds(firstResult),
    ...(secondResult ? claudeExternalMessageIds(secondResult) : []),
  ];
  const uniqueExternalMessageIds = messageIds.length > 1
    && new Set(messageIds).size === messageIds.length;
  const evidence = {
    firstTurn,
    secondTurn,
    uniqueExternalMessageIds,
    externalMessageIdCount: messageIds.length,
  };
  const summary = {
    schema_version: '1flowbase.claude-repository-history/v2',
    ok: firstTurn.ok && secondTurn.ok && uniqueExternalMessageIds,
    repository,
    model,
    expected,
    evidence,
    timeline_paths: [
      firstResult.pty?.timeline_path ?? null,
      secondResult?.pty?.timeline_path ?? null,
    ],
  };
  fs.writeFileSync(path.join(outputRoot, 'summary.json'), `${JSON.stringify(summary, null, 2)}\n`, { mode: 0o600 });
  return summary;
}

module.exports = {
  CLAUDE_REPOSITORY_FOLLOWUP_PROMPT,
  CLAUDE_REPOSITORY_HISTORY_PROMPT,
  GIT_HISTORY_COMMAND,
  claudeRepositoryHistoryInvocation,
  inspectClaudeRepositoryHistoryResult,
  inspectClaudeFollowupResult,
  readRepositoryGitHistory,
  runClaudeRepositoryHistorySmoke,
};
