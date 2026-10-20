const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { scanDiffText, main } = require('../core.js');

function sourceDiff(file, source) {
  const lines = source.split('\n');
  return [`+++ b/${file}`, `@@ -0,0 +1,${lines.length} @@`, ...lines.map((line) => `+${line}`)].join('\n');
}
function processes(diff, options) {
  const findings = scanDiffText(diff, options);
  assert.deepEqual(findings.filter((finding) => finding.kind === 'process-scan-incomplete'), [], 'valid fixtures must be scanned, not silently skipped');
  return findings.filter((finding) => finding.kind === 'process-execution');
}

test('JavaScript resolves child_process namespaces and named import/require aliases', () => {
  const source = [
    'import * as cp from "node:child_process";',
    'import legacy from "child_process";',
    'import { exec as run } from "node:child_process";',
    'const worker = require("child_process");',
    'const { spawn: launch, execFileSync } = require("node:child_process");',
    'const fork = worker.fork;',
    'cp.spawn("echo");',
    'legacy.execSync("echo");',
    'run("echo");',
    'launch("echo");',
    'execFileSync("echo");',
    'fork("worker.js");',
    'require("child_process").execFile("echo");',
    'cp["spawnSync"]("echo");',
  ].join('\n');
  assert.deepEqual(processes(sourceDiff('scripts/runner.ts', source)).map(({ line, api }) => [line, api]), [
    [7, 'child_process.spawn'], [8, 'child_process.execSync'], [9, 'child_process.exec'],
    [10, 'child_process.spawn'], [11, 'child_process.execFileSync'], [12, 'child_process.fork'],
    [13, 'child_process.execFile'], [14, 'child_process.spawnSync'],
  ]);
});

test('process imports, regex methods, task names, text and lexical shadows are not OS calls', () => {
  const source = [
    'import { exec as run } from "node:child_process";',
    'import * as cp from "node:child_process";',
    'const quoted = "cp.spawn(ignored)";',
    'const template = `run(ignored)`;',
    '// cp.exec("ignored");',
    '/* run("ignored"); */',
    '/word/.exec("word");',
    'new RegExp("word").exec("word");',
    'function shadow(run, cp) { run("ignored"); cp.spawn("ignored"); }',
    'function customRequire(require) { require("child_process").exec("ignored"); }',
    'const task = { spawn() {} }; task.spawn();',
    'function nested() { const cp = { exec() {} }; cp.exec(); }',
    'let changed = cp.spawn; changed = () => {}; changed();',
  ].join('\n');
  assert.deepEqual(processes(sourceDiff('scripts/runner.js', source)), []);
});

test('real calls embedded in template expressions remain visible', () => {
  const source = 'import { exec } from "child_process"; const value = `${exec("echo")}`;';
  assert.equal(processes(sourceDiff('runner.ts', source)).length, 1);
});

test('Rust resolves process Command paths/use aliases without tokio task/string/comment false positives', () => {
  const source = [
    'use std::process::Command;',
    'use tokio::process::{Command as AsyncCommand};',
    'use std::{process as proc};',
    'fn main() {',
    '  Command::new("echo");',
    '  AsyncCommand::new("echo");',
    '  proc::Command::new("echo");',
    '  std::process::Command::new("echo");',
    '  tokio::process::Command::new("echo");',
    '  tokio::spawn(async {});',
    '  let text = r###"Command::new(fake); std::process::Command::new(fake)"###;',
    '  // AsyncCommand::new("fake");',
    '  /* Command::new("fake"); */',
    '  { struct Command; Command::new("fake"); }',
    '}',
    'mod custom { struct Command; fn f() { Command::new("fake"); } }',
  ].join('\n');
  assert.deepEqual(processes(sourceDiff('api/worker.rs', source)).map(({ line }) => line), [5, 6, 7, 8, 9]);
});

test('changed-side hunk locations account for context, deletions and repeated calls', async () => {
  const diff = [
    '+++ b/runner.js', '@@ -1,3 +1,5 @@',
    ' const cp = require("child_process");',
    '-cp.exec("old");', '+cp.spawn("echo");',
    ' context();', '+cp.spawn("echo");', '+cp.spawn("echo");',
  ].join('\n');
  assert.deepEqual(processes(diff).map(({ line }) => line), [2, 4, 5]);
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'security-risk-lines-'));
  try {
    await main([], {
      repoRoot,
      env: { SECURITY_RISK_CHANGED_FILES: 'runner.js', SECURITY_RISK_DIFF: diff },
      writeStdout() {}, writeStderr() {},
    });
    const report = JSON.parse(fs.readFileSync(path.join(repoRoot, 'tmp/test-governance/security-risk.json'), 'utf8'));
    assert.deepEqual(report.findings.filter((finding) => finding.kind === 'process-execution').map(({ line }) => line), [2, 4, 5]);
  } finally { fs.rmSync(repoRoot, { recursive: true, force: true }); }
});

test('repository source resolves unchanged imports and lexical shadows across separate hunks', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'security-risk-context-'));
  try {
    fs.writeFileSync(path.join(repoRoot, 'runner.js'), [
      'import { spawn as launch } from "child_process";',
      'function shadow(launch) {', '  launch("fake");', '}',
      'launch("echo");',
    ].join('\n'));
    const diff = [
      '+++ b/runner.js', '@@ -3 +3 @@', '-  old();', '+  launch("fake");',
      '@@ -5 +5 @@', '-old();', '+launch("echo");',
    ].join('\n');
    assert.deepEqual(processes(diff, { repoRoot }).map(({ line, sourceContext }) => [line, sourceContext]), [[5, 'repository-file']]);
  } finally { fs.rmSync(repoRoot, { recursive: true, force: true }); }
});

test('multiline calls attach a changed argument line to the real call', () => {
  const diff = [
    '+++ b/runner.js', '@@ -1,4 +1,4 @@',
    ' const cp = require("child_process");', ' cp.spawn(',
    '-  "old",', '+  "echo",', ' );',
  ].join('\n');
  assert.deepEqual(processes(diff).map(({ line }) => line), [3]);
});

test('incomplete fragments and parser errors are explicit medium advisories', () => {
  const fragment = scanDiffText('+++ b/runner.ts\n@@ -20 +20 @@\n+launch("echo");');
  assert.ok(fragment.some((finding) => finding.kind === 'process-scan-incomplete' && finding.severity === 'medium'));
  const invalid = scanDiffText(sourceDiff('runner.ts', 'const cp = require("child_process");\ncp.spawn('));
  assert.ok(invalid.some((finding) => finding.kind === 'process-scan-incomplete' && /incomplete/u.test(finding.sample)));
});

test('vendor and test directories retain real process calls', () => {
  for (const file of ['api/vendor/tool.js', 'scripts/tool/_tests/fixture.js']) {
    assert.equal(processes(sourceDiff(file, 'const cp = require("child_process"); cp.spawn("echo");')).length, 1);
  }
});


test('Rust wildcard imports keep unknown Command identity advisory without declaring it an OS API', () => {
  const source = 'use super::*; fn start() { Command::new("worker"); }';
  const findings = scanDiffText(sourceDiff('api/worker.rs', source));
  assert.deepEqual(findings.filter((finding) => finding.kind === 'process-execution'), []);
  assert.ok(findings.some((finding) => finding.kind === 'process-api-unresolved' && finding.severity === 'medium' && finding.line === 1));
});

test('TypeScript import-equals aliases resolve the process namespace', () => {
  assert.equal(processes(sourceDiff('runner.cts', 'import cp = require("node:child_process"); cp.spawn("echo");')).length, 1);
});
