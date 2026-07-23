'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { executeTmuxInvocation, ptyMarkerTimeline, shellQuote } = require('../runner');

test('shell quoting preserves spaces, quotes, and shell metacharacters as one argument', () => {
  const value = "a b'c;$HOME";
  assert.equal(shellQuote(value), "'a b'\\''c;$HOME'");
});

test('advanced util-linux timing maps the two visible markers to distinct output times', () => {
  const output = 'prefix delta-1 middle delta-2 terminal';
  const firstBytes = Buffer.byteLength('prefix delta-1');
  const timing = `H 0.000000 START_TIME 0\nO 0.010000 ${firstBytes}\nO 0.250000 ${Buffer.byteLength(output) - firstBytes}\n`;
  assert.deepEqual(ptyMarkerTimeline(output, timing, ['delta-1', 'delta-2']), {
    'delta-1': 10,
    'delta-2': 260,
  });
});

test('tmux PTY runner records util-linux output and timing without using the user tmux server', async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'compatible-stream-tmux-'));
  try {
    const result = await executeTmuxInvocation({
      executable: '/usr/bin/printf',
      args: ['%s\n', '{"type":"result","result":"1flowbase gateway sentinel ok"}'],
      cwd: root,
    }, {
      PATH: process.env.PATH,
      HOME: root,
    }, {
      artifactDirectory: root,
      timeoutMs: 10_000,
    });

    assert.equal(result.exit_code, 0);
    assert.equal(result.timed_out, false);
    assert.match(result.stdout.text, /1flowbase gateway sentinel ok/u);
    assert.match(result.pty.timing, /(^|\n)O\s/u);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
