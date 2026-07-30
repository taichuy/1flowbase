'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const { narrowEnvironment } = require('./environment');
const { executeTmuxInvocation, parseJsonLines } = require('./runner');

const GIT_HISTORY_COMMAND = "git log -3 --format='%h %s'";
const CLAUDE_REPOSITORY_HISTORY_PROMPT = [
  '最近三次提交分别是什么。',
  `必须使用 Bash 执行只读命令 \`${GIT_HISTORY_COMMAND}\` 获取当前仓库事实。`,
  '最后只按命令输出顺序回答三行 `<短哈希> <原始提交标题>`，不得凭记忆或改写标题。',
].join(' ');

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

function claudeRepositoryHistoryInvocation(executable, paths, model) {
  fs.mkdirSync(paths.config, { recursive: true, mode: 0o700 });
  const settingsPath = path.join(paths.config, 'settings.json');
  fs.writeFileSync(settingsPath, '{}\n', { mode: 0o600 });
  return {
    executable,
    cwd: paths.repository,
    settingsPath,
    args: [
      '--bare',
      '-p', CLAUDE_REPOSITORY_HISTORY_PROMPT,
      '--no-session-persistence',
      '--settings', settingsPath,
      '--output-format', 'stream-json',
      '--include-partial-messages',
      '--verbose',
      '--model', model,
      '--tools', 'Bash',
      '--allowedTools', 'Bash(git log -3 --format=*)',
      '--permission-mode', 'dontAsk',
      '--disable-slash-commands',
      '--no-chrome',
    ],
  };
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
    && matchingToolUses.length > 0
    && unexpectedBashToolCalls === 0
    && completedGitToolCalls > 0
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
  const invocation = claudeRepositoryHistoryInvocation(
    options.claudeExecutable || process.env.CLAUDE_CODE_EXECUTABLE || 'claude',
    paths,
    model
  );
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
  const result = await (dependencies.executeTmuxInvocation || executeTmuxInvocation)(
    invocation,
    environment,
    {
      artifactDirectory: path.join(outputRoot, 'tmux'),
      timeoutMs: options.timeoutMs || 180000,
      secrets: [secret],
    }
  );
  const wireModel = model.replace(/\[1m\]$/iu, '');
  const evidence = inspectClaudeRepositoryHistoryResult(result, expected, wireModel);
  const summary = {
    schema_version: '1flowbase.claude-repository-history/v1',
    ok: evidence.ok,
    repository,
    model,
    expected,
    evidence,
    timeline_path: result.pty?.timeline_path ?? null,
  };
  fs.writeFileSync(path.join(outputRoot, 'summary.json'), `${JSON.stringify(summary, null, 2)}\n`, { mode: 0o600 });
  return summary;
}

module.exports = {
  CLAUDE_REPOSITORY_HISTORY_PROMPT,
  GIT_HISTORY_COMMAND,
  claudeRepositoryHistoryInvocation,
  inspectClaudeRepositoryHistoryResult,
  readRepositoryGitHistory,
  runClaudeRepositoryHistorySmoke,
};
