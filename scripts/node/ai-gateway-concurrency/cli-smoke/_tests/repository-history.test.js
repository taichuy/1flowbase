'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const {
  CLAUDE_REPOSITORY_FOLLOWUP_PROMPT,
  CLAUDE_REPOSITORY_HISTORY_PROMPT,
  GIT_HISTORY_COMMAND,
  claudeRepositoryHistoryInvocation,
  inspectClaudeRepositoryHistoryResult,
  readRepositoryGitHistory,
  runClaudeRepositoryHistorySmoke,
} = require('../repository-history');
const { parseArgs } = require('../../../cli/ai-gateway-cli-smoke');

test('CLI selects the native Claude repository-history mode without full matrix inputs', () => {
  assert.deepEqual(parseArgs([
    '--claude-repository-history',
    '--repository', '/home/taichuy/git/1flowbase',
    '--model', 'claude-opus-4-8[1M]',
    '--proxy-url', 'http://127.0.0.1:7897',
    '--evidence-root', '/tmp/history-evidence',
    '--timeout-ms', '90000',
  ], {}), {
    claudeRepositoryHistory: true,
    repository: '/home/taichuy/git/1flowbase',
    model: 'claude-opus-4-8[1M]',
    proxyUrl: 'http://127.0.0.1:7897',
    evidenceRoot: '/tmp/history-evidence',
    timeoutMs: 90000,
  });
});

test('repository history invocation pins native Claude Code, cwd, model, and Bash-only access', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'claude-repository-history-'));
  const config = path.join(root, 'config');
  const repository = path.join(root, 'repository');
  fs.mkdirSync(config);
  fs.mkdirSync(repository);

  const invocation = claudeRepositoryHistoryInvocation('/bin/claude', {
    config,
    repository,
  }, 'claude-opus-4-8[1M]', { sessionId: '11111111-1111-4111-8111-111111111111' });

  assert.equal(invocation.cwd, repository);
  assert.equal(invocation.executable, '/bin/claude');
  assert.equal(invocation.args[invocation.args.indexOf('-p') + 1], CLAUDE_REPOSITORY_HISTORY_PROMPT);
  assert.equal(invocation.args[invocation.args.indexOf('--model') + 1], 'claude-opus-4-8[1M]');
  assert.equal(
    invocation.args[invocation.args.indexOf('--session-id') + 1],
    '11111111-1111-4111-8111-111111111111',
  );
  assert.equal(invocation.args[invocation.args.indexOf('--tools') + 1], 'Bash');
  assert.equal(
    invocation.args[invocation.args.indexOf('--allowedTools') + 1],
    'Bash(git log -3 --format=*)'
  );
  assert.equal(invocation.args[invocation.args.indexOf('--permission-mode') + 1], 'dontAsk');
  assert.deepEqual(JSON.parse(fs.readFileSync(invocation.settingsPath, 'utf8')), {});

  const followup = claudeRepositoryHistoryInvocation('/bin/claude', {
    config,
    repository,
  }, 'claude-opus-4-8[1M]', {
    sessionId: '11111111-1111-4111-8111-111111111111',
    turnIndex: 1,
  });
  assert.equal(followup.args[followup.args.indexOf('-p') + 1], CLAUDE_REPOSITORY_FOLLOWUP_PROMPT);
  assert.equal(
    followup.args[followup.args.indexOf('--resume') + 1],
    '11111111-1111-4111-8111-111111111111',
  );
  assert.equal(followup.args.includes('--allowedTools'), false);
});

test('readRepositoryGitHistory builds a three-commit runtime oracle', () => {
  const calls = [];
  const commits = readRepositoryGitHistory('/repo', (command, args, options) => {
    calls.push({ command, args, options });
    return {
      status: 0,
      stdout: 'abc1234\tthird\ndef5678\tsecond\n987fedc\tfirst\n',
      stderr: '',
    };
  });

  assert.deepEqual(commits, [
    { shortHash: 'abc1234', subject: 'third' },
    { shortHash: 'def5678', subject: 'second' },
    { shortHash: '987fedc', subject: 'first' },
  ]);
  assert.deepEqual(calls[0].args, ['log', '-3', '--format=%h%x09%s']);
  assert.equal(calls[0].options.cwd, '/repo');
});

test('repository history result requires a completed Bash git-log call and exact ordered answer', () => {
  const expected = [
    { shortHash: 'abc1234', subject: 'third' },
    { shortHash: 'def5678', subject: 'second' },
    { shortHash: '987fedc', subject: 'first' },
  ];
  const answer = expected.map(({ shortHash, subject }) => `${shortHash} ${subject}`).join('\n');
  const stdout = [
    {
      type: 'stream_event',
      event: {
        type: 'message_start',
        message: { model: 'claude-opus-4-8' },
      },
    },
    {
      type: 'assistant',
      message: {
        content: [{
          type: 'tool_use', id: 'toolu_git', name: 'Bash',
          input: { command: GIT_HISTORY_COMMAND },
        }],
      },
    },
    {
      type: 'user',
      message: {
        content: [{ type: 'tool_result', tool_use_id: 'toolu_git', content: answer }],
      },
    },
    { type: 'assistant', message: { content: [{ type: 'text', text: answer }] } },
    { type: 'result', result: answer },
  ].map((event) => JSON.stringify(event)).join('\n');

  const evidence = inspectClaudeRepositoryHistoryResult({
    exit_code: 0,
    timed_out: false,
    stdout: { text: stdout, overflow: false },
    stderr: { text: '', overflow: false },
  }, expected, 'claude-opus-4-8');

  assert.deepEqual(evidence, {
    ok: true,
    expectedCount: 3,
    matchingBashToolCalls: 1,
    unexpectedBashToolCalls: 0,
    completedGitToolCalls: 1,
    matchedCommitCount: 3,
    exactOrder: true,
    exactAnswer: true,
    observedWireModels: ['claude-opus-4-8'],
    wireModelMatched: true,
    exitCode: 0,
    timedOut: false,
  });
});

test('repository history result rejects extra Bash operations even with an exact answer', () => {
  const expected = [
    { shortHash: 'abc1234', subject: 'third' },
    { shortHash: 'def5678', subject: 'second' },
    { shortHash: '987fedc', subject: 'first' },
  ];
  const answer = expected.map(({ shortHash, subject }) => `${shortHash} ${subject}`).join('\n');
  const stdout = [
    {
      type: 'assistant',
      message: {
        content: [{
          type: 'tool_use', id: 'toolu_unsafe', name: 'Bash',
          input: { command: `${GIT_HISTORY_COMMAND}; touch /tmp/not-read-only` },
        }],
      },
    },
    {
      type: 'user',
      message: { content: [{ type: 'tool_result', tool_use_id: 'toolu_unsafe', content: answer }] },
    },
    { type: 'result', result: answer },
  ].map((event) => JSON.stringify(event)).join('\n');

  const evidence = inspectClaudeRepositoryHistoryResult({
    exit_code: 0,
    timed_out: false,
    stdout: { text: stdout, overflow: false },
    stderr: { text: '', overflow: false },
  }, expected);

  assert.equal(evidence.ok, false);
  assert.equal(evidence.matchingBashToolCalls, 0);
  assert.equal(evidence.unexpectedBashToolCalls, 1);
});

test('repository history result rejects repeated matching git-log calls', () => {
  const expected = [
    { shortHash: 'abc1234', subject: 'third' },
    { shortHash: 'def5678', subject: 'second' },
    { shortHash: '987fedc', subject: 'first' },
  ];
  const answer = expected.map(({ shortHash, subject }) => `${shortHash} ${subject}`).join('\n');
  const toolUses = ['toolu_git_1', 'toolu_git_2'].map((id) => ({
    type: 'tool_use',
    id,
    name: 'Bash',
    input: { command: GIT_HISTORY_COMMAND },
  }));
  const stdout = [{
    type: 'assistant',
    message: { content: toolUses },
  }, {
    type: 'user',
    message: {
      content: toolUses.map(({ id }) => ({
        type: 'tool_result',
        tool_use_id: id,
        content: answer,
      })),
    },
  }, {
    type: 'result',
    result: answer,
  }].map((event) => JSON.stringify(event)).join('\n');

  const evidence = inspectClaudeRepositoryHistoryResult({
    exit_code: 0,
    timed_out: false,
    stdout: { text: stdout, overflow: false },
    stderr: { text: '', overflow: false },
  }, expected);

  assert.equal(evidence.ok, false);
  assert.equal(evidence.matchingBashToolCalls, 2);
  assert.equal(evidence.completedGitToolCalls, 2);
  assert.equal(evidence.unexpectedBashToolCalls, 0);
  assert.equal(evidence.exactAnswer, true);
});

test('repository history result rejects an answer without completed tool evidence', () => {
  const expected = [
    { shortHash: 'abc1234', subject: 'third' },
    { shortHash: 'def5678', subject: 'second' },
    { shortHash: '987fedc', subject: 'first' },
  ];
  const answer = expected.map(({ shortHash, subject }) => `${shortHash} ${subject}`).join('\n');
  const result = {
    exit_code: 0,
    timed_out: false,
    stdout: { text: JSON.stringify({ type: 'result', result: answer }), overflow: false },
    stderr: { text: '', overflow: false },
  };

  assert.equal(inspectClaudeRepositoryHistoryResult(result, expected).ok, false);
});

test('repository history smoke isolates credentials and writes secret-free evidence', async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'claude-repository-smoke-'));
  const repository = path.join(root, 'repository');
  const evidenceRoot = path.join(root, 'evidence');
  fs.mkdirSync(repository);
  const expected = [
    { shortHash: 'abc1234', subject: 'third' },
    { shortHash: 'def5678', subject: 'second' },
    { shortHash: '987fedc', subject: 'first' },
  ];
  const answer = expected.map(({ shortHash, subject }) => `${shortHash} ${subject}`).join('\n');
  const stdout = [
    {
      type: 'stream_event',
      event: { type: 'message_start', message: { id: 'msg-turn-1', model: 'claude-opus-4-8' } },
    },
    {
      type: 'assistant',
      message: {
        content: [{
          type: 'tool_use', id: 'toolu_git', name: 'Bash',
          input: { command: GIT_HISTORY_COMMAND },
        }],
      },
    },
    {
      type: 'user',
      message: { content: [{ type: 'tool_result', tool_use_id: 'toolu_git', content: answer }] },
    },
    { type: 'result', result: answer },
  ].map((event) => JSON.stringify(event)).join('\n');
  const captured = [];
  const followupStdout = [
    {
      type: 'stream_event',
      event: { type: 'message_start', message: { id: 'msg-turn-2', model: 'claude-opus-4-8' } },
    },
    { type: 'result', is_error: false, result: '透传协议由源协议上下文与目标 Provider 能力共同决定。' },
  ].map((event) => JSON.stringify(event)).join('\n');

  const summary = await runClaudeRepositoryHistorySmoke({
    repository,
    model: 'claude-opus-4-8[1M]',
    baseUrl: 'http://127.0.0.1:7800',
    authToken: 'secret-gateway-token',
    proxyUrl: 'http://127.0.0.1:7897',
    evidenceRoot,
    claudeExecutable: '/bin/claude',
    sessionId: '11111111-1111-4111-8111-111111111111',
  }, {
    parentEnv: { PATH: process.env.PATH },
    spawnSyncImpl: () => ({
      status: 0,
      stdout: expected.map(({ shortHash, subject }) => `${shortHash}\t${subject}`).join('\n') + '\n',
      stderr: '',
    }),
    executeTmuxInvocation: async (invocation, environment, options) => {
      captured.push({ invocation, environment, options });
      const followup = invocation.args.includes('--resume');
      return {
        exit_code: 0,
        timed_out: false,
        stdout: { text: followup ? followupStdout : stdout, overflow: false },
        stderr: { text: '', overflow: false },
        pty: { timeline_path: followup ? '/tmp/timeline-2.jsonl' : '/tmp/timeline-1.jsonl' },
      };
    },
  });

  assert.equal(summary.ok, true);
  assert.equal(summary.evidence.firstTurn.ok, true);
  assert.equal(summary.evidence.secondTurn.ok, true);
  assert.equal(summary.evidence.uniqueExternalMessageIds, true);
  assert.equal(captured.length, 2);
  assert.equal(captured[0].invocation.cwd, repository);
  assert.equal(captured[0].environment.ANTHROPIC_AUTH_TOKEN, 'secret-gateway-token');
  assert.equal(captured[0].environment.ANTHROPIC_API_KEY, '');
  assert.equal(captured[0].environment.CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC, '1');
  assert.equal(captured[0].environment.HTTP_PROXY, 'http://127.0.0.1:7897');
  assert.equal(captured[0].environment.HTTPS_PROXY, 'http://127.0.0.1:7897');
  assert.equal(captured[0].environment.http_proxy, 'http://127.0.0.1:7897');
  assert.equal(captured[0].environment.https_proxy, 'http://127.0.0.1:7897');
  assert.deepEqual(captured[0].options.secrets, ['secret-gateway-token']);
  assert.equal(captured[1].invocation.args.includes('--resume'), true);
  assert.doesNotMatch(fs.readFileSync(path.join(evidenceRoot, 'summary.json'), 'utf8'), /secret-gateway-token/u);
});
