#!/usr/bin/env node
'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { spawn, spawnSync } = require('node:child_process');
const { buildApiServerShardCommands, parseLlvmCovEnvironment } = require('./core.js');

function command(executable, args, { cwd, env } = {}) {
  const result = spawnSync(executable, args, { cwd, env, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `${executable} ${args.join(' ')}: ${(result.stderr || result.stdout).slice(-4000)}`);
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

function cacheCleanupProof(scratch, plan) {
  const cwd = path.join(scratch, 'api');
  const target = path.join(cwd, 'target');
  const retiredSource = path.join(cwd, 'apps/api-server/src/retired.rs');
  const files = {
    'Cargo.toml': '[workspace]\nmembers = ["apps/api-server", "crates/cache-proof-dependency"]\nresolver = "2"\n',
    'apps/api-server/Cargo.toml': '[package]\nname = "api-server"\nversion = "0.0.0"\nedition = "2021"\n'
      + '[dependencies]\ncache-proof-dependency = { path = "../../crates/cache-proof-dependency" }\n',
    'crates/cache-proof-dependency/Cargo.toml': '[package]\nname = "cache-proof-dependency"\nversion = "0.0.0"\nedition = "2021"\n',
    'crates/cache-proof-dependency/src/lib.rs': 'pub fn increment(n: u32) -> u32 { n + 1 }\n',
    'apps/api-server/src/lib.rs': [
      '#![allow(unexpected_cfgs)]',
      '#[cfg(proof_retired)] mod retired;',
      'pub fn classify(n: u32) -> u32 {',
      '    #[cfg(proof_retired)]',
      '    if n == 99 { return retired::classify(n); }',
      '    if n % 3 == 0 { cache_proof_dependency::increment(n) } else { n + 2 }',
      '}',
      '#[test] fn exercises_current() {',
      '    for n in [0, 1, 2, 3] { assert!(classify(n) > n); }',
      '}',
    ].join('\n'),
  };
  for (const [name, contents] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(cwd, name)), { recursive: true });
    fs.writeFileSync(path.join(cwd, name), contents);
  }
  // Neither inherited coverage paths nor Cargo's separate build directory may escape this probe.
  const baseEnv = Object.fromEntries(Object.entries(process.env).filter(([name]) =>
    !/^(?:CARGO_LLVM_COV|LLVM_|RUSTFLAGS$|CARGO_ENCODED_RUSTFLAGS$|RUSTC_WRAPPER$|RUSTC_WORKSPACE_WRAPPER$|CARGO_BUILD_TARGET$)/u.test(name)));
  Object.assign(baseEnv, {
    CARGO_TARGET_DIR: target, CARGO_BUILD_BUILD_DIR: target,
    CARGO_LLVM_COV_TARGET_DIR: target, CARGO_LLVM_COV_BUILD_DIR: target,
    LLVM_PROFILE_FILE_NAME: 'cache-proof-%p-%m.profraw', LLVM_PROFILE_FILE: path.join(target, 'cache-proof-%p-%m.profraw'),
    CARGO_NET_OFFLINE: 'true', CARGO_PROFILE_DEV_DEBUG: '0', CARGO_PROFILE_DEV_INCREMENTAL: 'false',
    RUSTFLAGS: '',
  });
  const showEnv = plan.find(({ args }) => args[0] === 'llvm-cov' && args[1] === 'show-env');
  const cleanup = plan.find(({ args }) => args[0] === 'clean');
  assert.deepEqual(cleanup?.args, ['clean', '--package', 'api-server']);
  assert.equal(cleanup.command, 'cargo');
  assert.equal(cleanup.cwd, cwd);
  assert.equal(showEnv?.cwd, cwd);
  const env = { ...baseEnv, ...parseLlvmCovEnvironment(command(showEnv.command, showEnv.args, { cwd, env: baseEnv })) };
  for (const key of ['CARGO_TARGET_DIR', 'CARGO_BUILD_BUILD_DIR', 'CARGO_LLVM_COV_TARGET_DIR', 'CARGO_LLVM_COV_BUILD_DIR']) {
    assert.equal(env[key], target, `${key} must remain inside the disposable workspace`);
  }
  assert.equal(path.dirname(env.LLVM_PROFILE_FILE), target);
  function build(buildEnv = env) {
    const artifacts = command('cargo', ['test', '--package', 'api-server', '--no-run', '--message-format=json'], { cwd, env: buildEnv })
      .split('\n').filter(Boolean).map((line) => JSON.parse(line))
      .filter((item) => item.reason === 'compiler-artifact' && item.profile.test && item.executable);
    assert.equal(artifacts.length, 1, 'probe must build exactly one test executable');
    return artifacts[0].executable;
  }
  function report(binary) {
    for (const name of fs.readdirSync(target).filter((file) => file.endsWith('.profraw'))) {
      fs.rmSync(path.join(target, name));
    }
    assert.match(command(binary, ['--exact', 'exercises_current'], { cwd, env }), /test result: ok\. 1 passed;/u);
    return JSON.parse(command('cargo', ['llvm-cov', 'report', '--package', 'api-server', '--json', '--summary-only'], { cwd, env }));
  }
  const currentBinary = build();
  const baseline = report(currentBinary);
  assert.ok(baseline.data[0].totals.lines.covered > 0, 'cache baseline must cover real source lines');
  fs.writeFileSync(retiredSource, [
    'pub fn classify(n: u32) -> u32 {',
    '    if n % 2 == 0 { n * 2 } else { n + 3 }',
    '}',
  ].join('\n'));
  const staleBinary = build({ ...env, RUSTFLAGS: `${env.RUSTFLAGS || ''} --cfg proof_retired`.trim() });
  assert.notEqual(staleBinary, currentBinary, 'changed compiler flags must leave a distinct cached executable');
  fs.rmSync(retiredSource);
  const rebuiltBinary = build();
  assert.equal(rebuiltBinary, currentBinary);
  assert.ok(fs.existsSync(staleBinary), 'current build must retain the stale cached executable');
  // Only the current executable runs; the old executable contributes mappings, never profiles.
  const contaminated = report(rebuiltBinary);
  const retiredCoverage = contaminated.data[0].files.find((file) => file.filename === retiredSource);
  assert.ok(retiredCoverage, 'cached executable must expose the deleted source');
  assert.equal(retiredCoverage.summary.lines.covered, 0, 'the retired source must have no execution profile');
  assert.ok(contaminated.data[0].totals.lines.count > baseline.data[0].totals.lines.count, 'cached executable must inflate the denominator');
  assert.notDeepEqual(contaminated, baseline, 'controlled cache contamination must be detected');
  function dependencyArtifacts() {
    const directory = path.join(target, 'debug/deps');
    return fs.readdirSync(directory).filter((name) => /^libcache_proof_dependency-.*\.rlib$/u.test(name)).sort().map((name) => {
      const contents = fs.readFileSync(path.join(directory, name));
      return { file: name, bytes: contents.length, sha256: createHash('sha256').update(contents).digest('hex') };
    });
  }
  const dependencies = dependencyArtifacts();
  assert.ok(dependencies.length > 0, 'probe must contain actual reusable dependency artifacts');
  command(cleanup.command, cleanup.args, { cwd: cleanup.cwd, env });
  assert.equal(fs.existsSync(staleBinary), false, 'native package cleanup must remove the stale executable');
  assert.deepEqual(dependencyArtifacts(), dependencies, 'native package cleanup must retain dependency artifacts unchanged');
  const cleaned = report(build());
  assert.deepEqual(cleaned, baseline, 'native package cleanup must restore the entire JSON summary, including every counter');
  return {
    nativeCleanup: [cleanup.command, ...cleanup.args], staleCacheDetected: true, staleExecutableRemoved: true,
    staleExecutable: path.relative(scratch, staleBinary), deletedSource: path.relative(scratch, retiredSource),
    identicalSummaryAfterCleanup: true, retainedDependencyArtifacts: dependencies,
    dependencyArtifactsPreserved: true, retainedDependencyCount: dependencies.length,
    retainedDependencyBytes: dependencies.reduce((sum, artifact) => sum + artifact.bytes, 0),
    summaries: { baseline, contaminated, cleaned },
  };
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
    const plan = buildApiServerShardCommands({
      repoRoot: scratch, shardIndex: 1, shardCount: 2, cargoTestThreads: workers,
    });
    const poolPattern = path.basename(plan.find((entry) => entry.profilePattern).profilePattern);
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
      cacheCleanup: cacheCleanupProof(scratch, plan),
    };
    fs.mkdirSync(path.dirname(outputPath), { recursive: true });
    fs.writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`);
    const { summaries, ...cacheCleanup } = report.cacheCleanup;
    process.stdout.write(`[coverage-profile-pool-proof] ${JSON.stringify({
      ...report, cacheCleanup: { ...cacheCleanup, lineCounts: Object.fromEntries(Object.entries(summaries)
        .map(([name, summary]) => [name, summary.data[0].totals.lines])) },
    })}\n`);
  } finally {
    fs.rmSync(scratch, { recursive: true, force: true });
  }
}

if (require.main === module) main(process.argv[2]).catch((error) => {
  process.stderr.write(`${error.stack}\n`);
  process.exitCode = 1;
});
