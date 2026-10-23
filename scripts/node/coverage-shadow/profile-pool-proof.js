#!/usr/bin/env node
'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawn, spawnSync } = require('node:child_process');
const { buildApiServerShardCommands } = require('./core.js');

function command(executable, args) {
  const result = spawnSync(executable, args, { encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `${executable}: ${result.stderr}`);
  return result.stdout.trim();
}

async function execute(binary, pattern, value) {
  await new Promise((resolve, reject) => {
    const child = spawn(binary, [String(value)], {
      env: { ...process.env, LLVM_PROFILE_FILE: pattern }, stdio: 'ignore',
    });
    child.once('error', reject);
    child.once('exit', (code) => code === 0 ? resolve() : reject(new Error(`probe exit ${code}`)));
  });
}

async function main(outputPath) {
  assert.ok(outputPath, 'output report path is required');
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'coverage-pool-proof-'));
  try {
    const host = /^host: (.+)$/mu.exec(command('rustc', ['-vV']))[1];
    const llvm = path.join(command('rustc', ['--print', 'sysroot']), 'lib/rustlib', host, 'bin');
    const source = path.join(scratch, 'probe.rs');
    const binary = path.join(scratch, 'probe');
    fs.writeFileSync(source, [
      '#[inline(never)] fn classify(n: u32) -> u32 {',
      '    if n % 3 == 0 { n * 2 } else if n % 3 == 1 { n + 1 } else { n - 1 }',
      '}',
      'fn main() {',
      '    let n = std::env::args().nth(1).unwrap().parse().unwrap();',
      '    println!("{}", classify(std::hint::black_box(n)));',
      '}',
    ].join('\n'));
    command('rustc', [source, '-C', 'instrument-coverage', '-C', 'debuginfo=0', '-o', binary]);
    const workers = os.availableParallelism();
    const runs = Math.max(32, workers * 4);
    const poolPattern = path.basename(buildApiServerShardCommands({
      repoRoot: scratch, shardIndex: 1, shardCount: 2, cargoTestThreads: workers,
    })[2].profilePattern);
    const profiles = {};
    const measurements = {};
    for (const [mode, pattern] of [['perProcess', 'shard-1-%p-%m.profraw'], ['pooled', poolPattern]]) {
      const directory = path.join(scratch, mode);
      fs.mkdirSync(directory);
      const started = performance.now();
      for (let offset = 0; offset < runs; offset += workers) {
        const batch = await Promise.allSettled(Array.from({ length: Math.min(workers, runs - offset) },
          (_, index) => execute(binary, path.join(directory, pattern), offset + index)));
        for (const item of batch) if (item.status === 'rejected') throw item.reason;
      }
      profiles[mode] = fs.readdirSync(directory).filter((file) => file.endsWith('.profraw'))
        .map((file) => path.join(directory, file));
      measurements[mode] = {
        pattern, executions: runs, milliseconds: performance.now() - started,
        profileCount: profiles[mode].length,
        profileBytes: profiles[mode].reduce((sum, file) => sum + fs.statSync(file).size, 0),
      };
    }
    function coverage(name, inputs) {
      const merged = path.join(scratch, `${name}.profdata`);
      command(path.join(llvm, 'llvm-profdata'), ['merge', '--sparse', ...inputs, '-o', merged]);
      return JSON.parse(command(path.join(llvm, 'llvm-cov'), ['export', binary, `--instr-profile=${merged}`]));
    }
    const original = coverage('per-process', profiles.perProcess);
    assert.ok(original.data[0].totals.lines.covered > 0, 'probe must exercise real instrumented lines');
    assert.deepEqual(coverage('pooled', profiles.pooled), original, 'pool must preserve all counters, files and regions');
    assert.notDeepEqual(coverage('lost-process', profiles.perProcess.slice(1)), original,
      'controlled loss of one execution must change the counters');
    assert.equal(measurements.perProcess.profileCount, runs);
    assert.ok(measurements.pooled.profileCount > 0 && measurements.pooled.profileCount <= workers);
    const report = {
      sha: process.env.QUALITY_GATE_TARGET_SHA, status: 'passed', workers, runs,
      identicalCoverage: true, controlledLossDetected: true, measurements,
    };
    fs.mkdirSync(path.dirname(outputPath), { recursive: true });
    fs.writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`);
    process.stdout.write(`[coverage-profile-pool-proof] ${JSON.stringify(report)}\n`);
  } finally {
    fs.rmSync(scratch, { recursive: true, force: true });
  }
}

if (require.main === module) main(process.argv[2]).catch((error) => {
  process.stderr.write(`${error.stack}\n`);
  process.exitCode = 1;
});
