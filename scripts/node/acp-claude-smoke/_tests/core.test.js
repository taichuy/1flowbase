const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { EventEmitter } = require('node:events');
const { PassThrough } = require('node:stream');

const {
  DEFAULT_ADAPTER_PACKAGE,
  DEFAULT_CLAUDE_EXECUTABLE,
  DEFAULT_NODE_BIN,
  DEFAULT_OUT_DIR,
  DEFAULT_PROMPT,
  GIT_HISTORY_COMMAND,
  GIT_HISTORY_PROMPT,
  parseCliArgs,
  runAcpClaudeSmoke,
  summarizeAcpEvidence,
} = require('../core.js');

test('parseCliArgs defaults to an ACP Claude Code thought smoke', () => {
  assert.deepEqual(parseCliArgs([]), {
    help: false,
    scenario: 'thought',
    prompt: DEFAULT_PROMPT,
    outDir: DEFAULT_OUT_DIR,
    cwd: null,
    adapterPackage: DEFAULT_ADAPTER_PACKAGE,
    nodeBin: DEFAULT_NODE_BIN,
    claudeExecutable: DEFAULT_CLAUDE_EXECUTABLE,
    model: null,
    effort: 'high',
    timeoutMs: 180000,
    maxThinkingTokens: '1024',
    useDefaultSettings: true,
    requireThought: true,
  });
});

test('parseCliArgs supports exploratory mode without requiring thought chunks', () => {
  const parsed = parseCliArgs([
    '--prompt',
    'hello',
    '--out-dir',
    'tmp/acp',
    '--cwd',
    'tmp/workspace',
    '--model',
    '1flowbase',
    '--no-effort',
    '--timeout-ms',
    '5000',
    '--max-thinking-tokens',
    '2048',
    '--no-default-settings',
    '--allow-missing-thought',
  ]);

  assert.equal(parsed.prompt, 'hello');
  assert.equal(parsed.outDir, 'tmp/acp');
  assert.equal(parsed.cwd, 'tmp/workspace');
  assert.equal(parsed.model, '1flowbase');
  assert.equal(parsed.effort, null);
  assert.equal(parsed.timeoutMs, 5000);
  assert.equal(parsed.maxThinkingTokens, '2048');
  assert.equal(parsed.useDefaultSettings, false);
  assert.equal(parsed.requireThought, false);
});

test('parseCliArgs selects the fixed repository git-history scenario', () => {
  const parsed = parseCliArgs([
    '--scenario',
    'git-history',
    '--cwd',
    '/home/taichuy/git/1flowbase',
    '--model',
    'opus',
  ]);

  assert.equal(parsed.scenario, 'git-history');
  assert.equal(parsed.prompt, GIT_HISTORY_PROMPT);
  assert.equal(parsed.model, 'opus');
  assert.equal(parsed.useDefaultSettings, false);
  assert.equal(parsed.requireThought, false);
});

test('summarizeAcpEvidence requires both thought and message chunks', () => {
  const summary = summarizeAcpEvidence({
    cwd: '/repo',
    prompt: 'hi',
    paths: {
      rawInPath: path.join('/repo', 'in.jsonl'),
      rawOutPath: path.join('/repo', 'out.jsonl'),
      stderrPath: path.join('/repo', 'stderr.log'),
      summaryPath: path.join('/repo', 'summary.json'),
    },
    agentRequests: [],
    errors: [],
    updates: [
      {
        update: {
          sessionUpdate: 'agent_thought_chunk',
          content: { type: 'text', text: '先分析' },
        },
      },
      {
        update: {
          sessionUpdate: 'agent_message_chunk',
          content: { type: 'text', text: '最终回答' },
        },
      },
    ],
    notifications: [
      {
        method: '_claude/sdkMessage',
        params: {
          message: {
            type: 'stream_event',
            event: {
              type: 'content_block_delta',
              delta: { type: 'thinking_delta', thinking: '先分析' },
            },
          },
        },
      },
    ],
    extra: {},
  });

  assert.equal(summary.ok, true);
  assert.equal(summary.updateCounts.agent_thought_chunk, 1);
  assert.equal(summary.updateCounts.agent_message_chunk, 1);
  assert.equal(summary.thoughtChars, '先分析'.length);
  assert.equal(summary.messageChars, '最终回答'.length);
  assert.equal(summary.rawThinkingDeltas, 1);
});

test('summarizeAcpEvidence accepts git history only with a Bash git-log call and exact ordered commits', () => {
  const expectedGitHistory = [
    { shortHash: 'ca8fff0', subject: "Merge branch 'dev' into beta" },
    { shortHash: '7f4737f', subject: 'feat(settings): add optional proxy URL field in model provider edit mode' },
    { shortHash: '16feea9', subject: 'Refine sign-in hero animation and panel layout' },
  ];
  const message = expectedGitHistory.map(({ shortHash, subject }) => `${shortHash} ${subject}`).join('\n');
  const summary = summarizeAcpEvidence({
    cwd: '/home/taichuy/git/1flowbase',
    prompt: GIT_HISTORY_PROMPT,
    scenario: 'git-history',
    expectedGitHistory,
    paths: {
      rawInPath: '/tmp/in.jsonl',
      rawOutPath: '/tmp/out.jsonl',
      stderrPath: '/tmp/stderr.log',
      summaryPath: '/tmp/summary.json',
    },
    agentRequests: [],
    errors: [],
    updates: [{
      update: {
        sessionUpdate: 'agent_message_chunk',
        content: { type: 'text', text: message },
      },
    }],
    notifications: [{
      method: '_claude/sdkMessage',
      params: {
        message: {
          type: 'assistant',
          message: {
            content: [{
              type: 'tool_use',
              id: 'toolu_git',
              name: 'Bash',
              input: { command: GIT_HISTORY_COMMAND },
            }],
          },
        },
      },
    }, {
      method: '_claude/sdkMessage',
      params: {
        message: {
          type: 'user',
          message: {
            content: [{ type: 'tool_result', tool_use_id: 'toolu_git', content: message }],
          },
        },
      },
    }],
    extra: {},
  });

  assert.equal(summary.ok, true);
  assert.deepEqual(summary.gitHistoryEvidence, {
    expectedCount: 3,
    matchingBashToolCalls: 1,
    unexpectedBashToolCalls: 0,
    completedGitToolCalls: 1,
    matchedCommitCount: 3,
    exactOrder: true,
    exactAnswer: true,
  });
});

test('summarizeAcpEvidence rejects extra Bash operations even with an exact answer', () => {
  const expectedGitHistory = [
    { shortHash: 'aaa1111', subject: 'third' },
    { shortHash: 'bbb2222', subject: 'second' },
    { shortHash: 'ccc3333', subject: 'first' },
  ];
  const message = expectedGitHistory
    .map(({ shortHash, subject }) => `${shortHash} ${subject}`)
    .join('\n');
  const summary = summarizeAcpEvidence({
    cwd: '/repo',
    prompt: GIT_HISTORY_PROMPT,
    scenario: 'git-history',
    expectedGitHistory,
    paths: {},
    agentRequests: [],
    errors: [],
    updates: [{
      update: {
        sessionUpdate: 'agent_message_chunk',
        content: { type: 'text', text: message },
      },
    }],
    notifications: [{
      method: '_claude/sdkMessage',
      params: {
        message: {
          type: 'assistant',
          message: {
            content: [{
              type: 'tool_use',
              id: 'toolu_unsafe',
              name: 'Bash',
              input: { command: `${GIT_HISTORY_COMMAND}; touch /tmp/not-read-only` },
            }],
          },
        },
      },
    }],
    extra: {},
  });

  assert.equal(summary.ok, false);
  assert.equal(summary.gitHistoryEvidence.matchingBashToolCalls, 0);
  assert.equal(summary.gitHistoryEvidence.unexpectedBashToolCalls, 1);
});

test('summarizeAcpEvidence rejects repeated matching git-log calls', () => {
  const expectedGitHistory = [
    { shortHash: 'aaa1111', subject: 'third' },
    { shortHash: 'bbb2222', subject: 'second' },
    { shortHash: 'ccc3333', subject: 'first' },
  ];
  const message = expectedGitHistory
    .map(({ shortHash, subject }) => `${shortHash} ${subject}`)
    .join('\n');
  const toolUses = ['toolu_git_1', 'toolu_git_2'].map((id) => ({
    type: 'tool_use',
    id,
    name: 'Bash',
    input: { command: GIT_HISTORY_COMMAND },
  }));
  const summary = summarizeAcpEvidence({
    cwd: '/repo',
    prompt: GIT_HISTORY_PROMPT,
    scenario: 'git-history',
    expectedGitHistory,
    paths: {},
    agentRequests: [],
    errors: [],
    updates: [{
      update: {
        sessionUpdate: 'agent_message_chunk',
        content: { type: 'text', text: message },
      },
    }],
    notifications: [{
      method: '_claude/sdkMessage',
      params: {
        message: { type: 'assistant', message: { content: toolUses } },
      },
    }, {
      method: '_claude/sdkMessage',
      params: {
        message: {
          type: 'user',
          message: {
            content: toolUses.map(({ id }) => ({
              type: 'tool_result',
              tool_use_id: id,
              content: message,
            })),
          },
        },
      },
    }],
    extra: {},
  });

  assert.equal(summary.ok, false);
  assert.equal(summary.gitHistoryEvidence.matchingBashToolCalls, 2);
  assert.equal(summary.gitHistoryEvidence.completedGitToolCalls, 2);
  assert.equal(summary.gitHistoryEvidence.unexpectedBashToolCalls, 0);
  assert.equal(summary.gitHistoryEvidence.exactAnswer, true);
});

test('summarizeAcpEvidence rejects a memorized git-history answer without tool evidence', () => {
  const expectedGitHistory = [
    { shortHash: 'aaa1111', subject: 'third' },
    { shortHash: 'bbb2222', subject: 'second' },
    { shortHash: 'ccc3333', subject: 'first' },
  ];
  const summary = summarizeAcpEvidence({
    cwd: '/repo',
    prompt: GIT_HISTORY_PROMPT,
    scenario: 'git-history',
    expectedGitHistory,
    paths: {},
    agentRequests: [],
    errors: [],
    updates: [{
      update: {
        sessionUpdate: 'agent_message_chunk',
        content: {
          type: 'text',
          text: expectedGitHistory.map(({ shortHash, subject }) => `${shortHash} ${subject}`).join('\n'),
        },
      },
    }],
    notifications: [],
    extra: {},
  });

  assert.equal(summary.ok, false);
  assert.equal(summary.gitHistoryEvidence.matchingBashToolCalls, 0);
  assert.equal(summary.gitHistoryEvidence.exactOrder, true);
});

test('runAcpClaudeSmoke returns timeout evidence when the ACP adapter stops responding', { timeout: 500 }, async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'acp-claude-timeout-'));
  const adapter = path.join(repoRoot, 'adapter.js');
  fs.writeFileSync(adapter, '');
  const child = new EventEmitter();
  child.stdin = {
    write() {},
    end() {},
  };
  child.stdout = new PassThrough();
  child.stderr = new PassThrough();
  let killCount = 0;
  let invocation = null;
  child.kill = () => {
    killCount += 1;
  };

  const summary = await runAcpClaudeSmoke(
    parseCliArgs(['--adapter', adapter, '--out-dir', 'out', '--timeout-ms', '20']),
    {
      repoRoot,
      spawnImpl: (command, args) => {
        invocation = { command, args };
        return child;
      },
    }
  );

  assert.equal(summary.ok, false);
  assert.equal(summary.timedOut, true);
  assert.deepEqual(invocation, { command: process.execPath, args: [adapter] });
  assert.ok(killCount > 0);
  assert.ok(fs.existsSync(path.join(repoRoot, 'out', 'acp-claude-summary.json')));
});
