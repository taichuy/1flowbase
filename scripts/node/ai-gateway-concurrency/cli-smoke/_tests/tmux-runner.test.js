'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const {
  assertCompatibleResult,
  executeTmuxInvocation,
  ptyMarkerTimeline,
  shellQuote,
} = require('../runner');

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

test('advanced util-linux timing excludes the output-log header from child byte offsets', () => {
  const header = 'Script started on 2026-07-23 22:00:00+08:00 [COMMAND="fixture"]\n';
  const childOutput = 'prefix delta-1 middle delta-2 terminal';
  const firstBytes = Buffer.byteLength('prefix delta-1');
  const timing = `H 0.000000 START_TIME 0\nO 0.010000 ${firstBytes}\nO 0.250000 ${Buffer.byteLength(childOutput) - firstBytes}\n`;
  assert.deepEqual(ptyMarkerTimeline(`${header}${childOutput}`, timing, ['delta-1', 'delta-2']), {
    'delta-1': 10,
    'delta-2': 260,
  });
});

test('OpenCode TUI acceptance reads the sentinel from raw PTY output', () => {
  const eventCount = assertCompatibleResult('opencode', {
    timed_out: false,
    exit_code: 0,
    stdout: { text: '\u001b[32m1flowbase gateway sentinel ok\u001b[0m', overflow: false },
    stderr: { text: '', overflow: false },
  });
  assert.equal(eventCount, 1);
});

test('tmux PTY runner records util-linux output and timing without using the user tmux server', async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'compatible-stream-tmux-'));
  let firstMarkerObserved = false;
  try {
    const result = await executeTmuxInvocation({
      executable: '/usr/bin/printf',
      args: ['%s\n', '{"type":"result","result":"marker-1 marker-2 1flowbase gateway sentinel ok"}'],
      cwd: root,
    }, {
      PATH: process.env.PATH,
      HOME: root,
    }, {
      artifactDirectory: root,
      timeoutMs: 10_000,
      markers: ['marker-1', 'marker-2'],
      onFirstMarker: () => { firstMarkerObserved = true; },
    });

    assert.equal(result.exit_code, 0);
    assert.equal(result.timed_out, false);
    assert.equal(firstMarkerObserved, true);
    assert.match(result.stdout.text, /1flowbase gateway sentinel ok/u);
    assert.match(result.pty.timing, /(^|\n)O\s/u);
    assert.equal(result.pty.observation, 'util-linux-script-raw-pty');
    const marker = fs.readFileSync(result.pty.timeline_path, 'utf8')
      .split('\n')
      .filter(Boolean)
      .map((line) => JSON.parse(line))
      .find((event) => event.event === 'marker_1');
    assert.equal(marker.source, 'util-linux-script-raw-pty');
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
