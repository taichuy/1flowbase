const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { backendThresholds } = require('../testing/coverage-thresholds.js');

const API_SERVER_PACKAGE = 'api-server';
const API_SERVER_SOURCE_MARKER = '/api/apps/api-server/src/';
const API_SERVER_LINE_THRESHOLD = backendThresholds.find(
  (entry) => entry.packageName === API_SERVER_PACKAGE
)?.line;
const ANSI_CONTROL_SEQUENCE_PATTERN = /\u001b(?:\[[0-?]*[ -/]*[@-~]|\][^\u0007]*(?:\u0007|\u001b\\)|[@-Z\\-_])/gu;

function assertShard(shardIndex, shardCount) {
  if (!Number.isInteger(shardIndex) || !Number.isInteger(shardCount)
    || shardCount < 2 || shardIndex < 1 || shardIndex > shardCount) {
    throw new Error(`invalid coverage shard ${shardIndex}/${shardCount}`);
  }
}

function buildApiServerShardCommands({ repoRoot, shardIndex, shardCount, cargoTestThreads }) {
  assertShard(shardIndex, shardCount);
  const shardDir = path.join(repoRoot, 'tmp', 'test-governance', 'coverage-shadow', 'api-server', `shard-${shardIndex}`);
  const partition = `hash:${shardIndex}/${shardCount}`;
  const common = ['--package', API_SERVER_PACKAGE, '--partition', partition];

  return [
    { command: 'cargo', args: ['llvm-cov', 'show-env', '--sh'], cwd: path.join(repoRoot, 'api') },
    { command: 'cargo', args: ['nextest', 'list', ...common, '--message-format', 'json'], cwd: path.join(repoRoot, 'api') },
    {
      command: 'cargo',
      args: ['nextest', 'run', ...common, '--test-threads', String(cargoTestThreads), '--no-fail-fast', '--no-tests=fail'],
      cwd: path.join(repoRoot, 'api'),
      // LLVM merges concurrent processes into a worker-sized pool per binary.
      // Including %p would instead retain a full profile for every test process.
      profilePattern: path.join(shardDir, `shard-${shardIndex}-%${cargoTestThreads}m.profraw`),
    },
  ];
}

function parseLlvmCovEnvironment(stdout) {
  const env = {};
  for (const line of stdout.replace(ANSI_CONTROL_SEQUENCE_PATTERN, '').split(/\r?\n/u)) {
    const match = line.match(/^export (?<key>[A-Z0-9_]+)=(?:'(?<quoted>.*)'|(?<bare>[^'].*))$/u);
    if (match) {
      env[match.groups.key] = (match.groups.quoted ?? match.groups.bare).replace(/'\\''/gu, "'");
    }
  }
  if (!env.RUSTC_WRAPPER || !env.CARGO_LLVM_COV) {
    throw new Error('cargo llvm-cov show-env did not return the required instrumentation environment');
  }
  return env;
}

function collectNextestTestIds(inventory, { ignored = false } = {}) {
  const suites = inventory?.['rust-suites'];
  if (!suites || typeof suites !== 'object') throw new Error('invalid nextest JSON inventory');
  return Object.entries(suites)
    .flatMap(([suiteId, suite]) => Object.entries(suite?.testcases || {})
      .filter(([, testcase]) => (testcase?.ignored === true) === ignored)
      // nextest checks ignored before partitioning. Ignored entries appear in
      // every shard, with filter-match mismatch/ignored, and never count as run.
      .filter(([, testcase]) => ignored || testcase?.['filter-match']?.status !== 'mismatch')
      .map(([testId]) => `${suiteId}::${testId}`))
    .sort();
}

function validateShardInventories({ fullInventory, shardInventories }) {
  const fullIds = collectNextestTestIds(fullInventory);
  if (fullIds.length === 0) throw new Error('coverage full inventory has no runnable tests');
  const expected = new Set(fullIds);
  const fullIgnoredIds = collectNextestTestIds(fullInventory, { ignored: true });
  const seen = new Set();
  const duplicates = [];
  for (const shardInventory of shardInventories) {
    for (const testId of collectNextestTestIds(shardInventory)) {
      if (seen.has(testId)) duplicates.push(testId);
      seen.add(testId);
    }
    if (JSON.stringify(collectNextestTestIds(shardInventory, { ignored: true })) !== JSON.stringify(fullIgnoredIds)) {
      throw new Error('coverage shard ignored inventory does not match full inventory');
    }
  }
  const missing = fullIds.filter((testId) => !seen.has(testId));
  const extra = [...seen].filter((testId) => !expected.has(testId));
  if (duplicates.length || missing.length || extra.length) {
    throw new Error(`coverage shard inventory mismatch: duplicate=${duplicates.length}, missing=${missing.length}, extra=${extra.length}`);
  }
  return {
    fullCount: fullIds.length,
    ignoredCount: fullIgnoredIds.length,
    shardCount: shardInventories.length,
  };
}

function parseNextestSummary(output) {
  const summaries = [...output.replace(ANSI_CONTROL_SEQUENCE_PATTERN, '').matchAll(
    /^\s*Summary\s+\[[^\]\r\n]+\]\s+(\d+) tests? run:\s*([^\r\n]*)$/gmu
  )];
  if (summaries.length !== 1) throw new Error('coverage shard requires one nextest execution summary');
  const [, total, details] = summaries[0];
  const passedCount = Number(details.match(/\b(\d+) passed\b/u)?.[1] ?? 0);
  const failedCount = Number(details.match(/\b(\d+) failed\b/u)?.[1] ?? 0);
  const testCount = Number(total);
  if (!(testCount > 0 && passedCount === testCount && failedCount === 0)) {
    throw new Error('coverage shard has no complete nonzero passing nextest execution');
  }
  // nextest skipped includes partition/filter exclusions; it is not ignored.
  return { testCount, passedCount, failedCount };
}

function shardProfiles(directory, shardIndex) {
  const profiles = fs.readdirSync(directory)
    .filter((name) => name.startsWith(`shard-${shardIndex}-`) && name.endsWith('.profraw'));
  for (const profile of profiles) {
    const info = fs.statSync(path.join(directory, profile));
    if (!info.isFile() || info.size === 0) throw new Error(`coverage shard has an empty or invalid profile: ${profile}`);
  }
  if (profiles.length === 0) throw new Error(`coverage shard ${shardIndex} produced no profraw files`);
  return profiles;
}

function validateShardMetadata({ metadata, inventory, shardIndex, shardCount, expectedSha, directory }) {
  if (!expectedSha || metadata?.sha !== expectedSha) {
    throw new Error('coverage shards do not belong to the same frozen SHA');
  }
  if (metadata.package !== API_SERVER_PACKAGE || metadata.partition !== `hash:${shardIndex}/${shardCount}`) {
    throw new Error(`coverage shard ${shardIndex} metadata package or partition mismatch`);
  }
  const testCount = collectNextestTestIds(inventory).length;
  const ignoredCount = collectNextestTestIds(inventory, { ignored: true }).length;
  if (testCount === 0 || metadata.testCount !== testCount || metadata.ignoredCount !== ignoredCount
    || metadata.exitCode !== 0 || metadata.passedCount !== testCount || metadata.failedCount !== 0) {
    throw new Error(`coverage shard ${shardIndex} metadata does not prove complete nonzero passing execution`);
  }
  const profiles = shardProfiles(directory, shardIndex);
  if (metadata.profileCount !== profiles.length) {
    throw new Error(`coverage shard ${shardIndex} metadata profile count mismatch`);
  }
  return profiles;
}

function canonicalCoverageSummary(summary) {
  const data = summary?.data?.[0];
  if (!data?.totals || !Array.isArray(data.files)) throw new Error('invalid cargo llvm-cov JSON summary');
  return {
    totals: data.totals,
    files: data.files
      .map((file) => ({ filename: file.filename, summary: file.summary }))
      .sort((left, right) => left.filename.localeCompare(right.filename)),
  };
}

function validateCoverageSummary(summary) {
  const coverage = canonicalCoverageSummary(summary);
  const structuralMetrics = Object.keys(coverage.totals).sort();
  const ownedLines = coverage.files
    .filter((file) => file.filename.replaceAll('\\', '/').includes(API_SERVER_SOURCE_MARKER))
    .map((file) => file.summary.lines);
  if (ownedLines.length === 0) {
    throw new Error('merged API coverage contains no API-owned source files');
  }
  if (ownedLines.some((lines) => !Number.isInteger(lines?.count) || !Number.isInteger(lines?.covered)
    || lines.count < 0 || lines.covered < 0 || lines.covered > lines.count)) {
    throw new Error('merged API coverage contains invalid API-owned line counts');
  }
  const lines = ownedLines.reduce((total, current) => ({
    count: total.count + (current?.count ?? 0),
    covered: total.covered + (current?.covered ?? 0),
  }), { count: 0, covered: 0 });
  if (lines.count === 0) throw new Error('merged API coverage contains no API-owned executable lines');
  const lineCoveragePercent = (lines.covered / lines.count) * 100;
  if (!Number.isFinite(API_SERVER_LINE_THRESHOLD)
    || lineCoveragePercent < API_SERVER_LINE_THRESHOLD) {
    throw new Error(
      `merged API coverage lines ${lineCoveragePercent.toFixed(2)}% is below ${Number(API_SERVER_LINE_THRESHOLD).toFixed(2)}%`
    );
  }
  return {
    fileCount: coverage.files.length,
    structuralMetrics,
    lineCoveragePercent,
    minimumLineCoveragePercent: API_SERVER_LINE_THRESHOLD,
  };
}

function compareCoverageSummaries(monolithic, merged) {
  const expected = canonicalCoverageSummary(monolithic);
  const actual = canonicalCoverageSummary(merged);
  const validation = validateCoverageSummary(merged);
  const { structuralMetrics } = validation;
  for (const metric of structuralMetrics) {
    if (expected.totals[metric]?.count !== actual.totals[metric]?.count) {
      throw new Error(`coverage shadow mismatch in ${metric} denominator`);
    }
  }
  const expectedFiles = new Map(expected.files.map((file) => [file.filename, file.summary]));
  const actualFiles = new Map(actual.files.map((file) => [file.filename, file.summary]));
  if (JSON.stringify([...expectedFiles.keys()]) !== JSON.stringify([...actualFiles.keys()])) {
    throw new Error('coverage shadow mismatch in file inventory');
  }
  let nondeterministicFiles = 0;
  for (const [filename, expectedSummary] of expectedFiles) {
    const actualSummary = actualFiles.get(filename);
    for (const metric of Object.keys(expectedSummary)) {
      if (expectedSummary[metric]?.count !== actualSummary?.[metric]?.count) {
        throw new Error(`coverage shadow mismatch in per-file ${metric} denominator: ${filename}`);
      }
    }
    if (JSON.stringify(expectedSummary) !== JSON.stringify(actualSummary)) nondeterministicFiles += 1;
  }
  const coveredDeltas = Object.fromEntries(structuralMetrics.map((metric) => [
    metric,
    (actual.totals[metric]?.covered ?? 0) - (expected.totals[metric]?.covered ?? 0),
  ]));
  return {
    ...validation,
    nondeterministicFiles,
    coveredDeltas,
  };
}

function run(command, {
  env, spawnSyncImpl = spawnSync, capture = false, logPrefix,
  writeStdout = (text) => process.stdout.write(text),
  writeStderr = (text) => process.stderr.write(text),
} = {}) {
  // File-backed capture retains raw failure output without spawnSync truncation.
  const outputFiles = logPrefix ? [`${logPrefix}.stdout.log`, `${logPrefix}.stderr.log`] : [];
  const outputFds = [];
  let result;
  try {
    for (const file of outputFiles) outputFds.push(fs.openSync(file, 'w'));
    result = spawnSyncImpl(command.command, command.args, {
      cwd: command.cwd,
      env,
      encoding: 'utf8',
      maxBuffer: 64 * 1024 * 1024,
      stdio: logPrefix ? ['ignore', ...outputFds] : capture ? 'pipe' : 'inherit',
    });
  } finally {
    for (const fd of outputFds) fs.closeSync(fd);
  }
  let stdout = result.stdout || '';
  let stderr = result.stderr || '';
  if (logPrefix) {
    // Keep raw output even on failure; no successful metadata is written then.
    stdout = fs.readFileSync(outputFiles[0], 'utf8');
    stderr = fs.readFileSync(outputFiles[1], 'utf8');
    writeStdout(stdout);
    writeStderr(stderr);
  }
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command.command} ${command.args.join(' ')} failed with exit code ${result.status ?? 1}`);
  return logPrefix ? `${stdout}\n${stderr}` : stdout;
}

function runApiServerShard({
  repoRoot, shardIndex, shardCount, cargoTestThreads, env = process.env, spawnSyncImpl, writeStdout, writeStderr,
}) {
  const commands = buildApiServerShardCommands({ repoRoot, shardIndex, shardCount, cargoTestThreads });
  const shardDir = path.dirname(commands[2].profilePattern);
  fs.rmSync(shardDir, { recursive: true, force: true });
  fs.mkdirSync(shardDir, { recursive: true });
  if (!env.QUALITY_GATE_TARGET_SHA?.trim()) throw new Error('coverage shard requires a frozen SHA');
  const llvmEnv = parseLlvmCovEnvironment(run(commands[0], { env, spawnSyncImpl, capture: true }));
  const instrumentedEnv = {
    ...env,
    ...llvmEnv,
    LLVM_PROFILE_FILE: commands[2].profilePattern,
    CARGO_PROFILE_TEST_DEBUG: '0',
    RUST_MIN_STACK: String(8 * 1024 * 1024),
  };
  const inventoryText = run(commands[1], { env: instrumentedEnv, spawnSyncImpl, capture: true });
  const inventory = JSON.parse(inventoryText);
  fs.writeFileSync(path.join(shardDir, `inventory-${shardIndex}.json`), `${JSON.stringify(inventory)}\n`, 'utf8');
  const testCount = collectNextestTestIds(inventory).length;
  if (testCount === 0) throw new Error('coverage shard inventory has no runnable tests');
  const execution = parseNextestSummary(run(commands[2], {
    env: instrumentedEnv, spawnSyncImpl, writeStdout, writeStderr,
    logPrefix: path.join(shardDir, `nextest-${shardIndex}`),
  }));
  if (execution.testCount !== testCount) throw new Error('coverage shard executed count does not match its inventory');
  const profiles = shardProfiles(shardDir, shardIndex);
  fs.writeFileSync(path.join(shardDir, `metadata-${shardIndex}.json`), `${JSON.stringify({
    package: API_SERVER_PACKAGE,
    partition: `hash:${shardIndex}/${shardCount}`,
    sha: env.QUALITY_GATE_TARGET_SHA || '',
    testCount,
    ignoredCount: collectNextestTestIds(inventory, { ignored: true }).length,
    exitCode: 0,
    passedCount: execution.passedCount,
    failedCount: execution.failedCount,
    profileCount: profiles.length,
  }, null, 2)}\n`, 'utf8');
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function mergeApiServerShadow({
  repoRoot,
  shardCount,
  monolithicPath,
  monolithicShaPath,
  env = process.env,
  spawnSyncImpl,
}) {
  assertShard(1, shardCount);
  const root = path.join(repoRoot, 'tmp', 'test-governance', 'coverage-shadow', 'api-server');
  const shardRoot = path.join(root, 'downloaded');
  const mergedPath = path.join(root, 'api-server-merged.json');
  const evidencePath = path.join(root, 'equivalence.json');
  const coveragePath = path.join(repoRoot, 'tmp', 'test-governance', 'coverage', 'backend', 'api-server.json');
  for (const outputPath of [mergedPath, evidencePath, coveragePath]) fs.rmSync(outputPath, { force: true });
  const inventories = [];
  const metadata = [];
  for (let index = 1; index <= shardCount; index += 1) {
    inventories.push(readJson(path.join(shardRoot, `inventory-${index}.json`)));
    metadata.push(readJson(path.join(shardRoot, `metadata-${index}.json`)));
  }
  const shas = new Set(metadata.map((item) => item?.sha));
  const expectedSha = env.QUALITY_GATE_TARGET_SHA || '';
  const monolithicSha = monolithicShaPath
    ? fs.readFileSync(monolithicShaPath, 'utf8').trim()
    : expectedSha;
  if (!expectedSha.trim() || shas.size !== 1 || !shas.has(expectedSha) || monolithicSha !== expectedSha) {
    throw new Error('coverage shards do not belong to the same frozen SHA');
  }
  const profiles = metadata.flatMap((item, offset) => validateShardMetadata({
    metadata: item, inventory: inventories[offset], shardIndex: offset + 1,
    shardCount, expectedSha, directory: shardRoot,
  }));
  if (fs.readdirSync(shardRoot).filter((name) => name.endsWith('.profraw')).length !== profiles.length) {
    throw new Error('coverage shadow contains unexpected shard profiles');
  }

  const apiCwd = path.join(repoRoot, 'api');
  const targetDir = path.resolve(apiCwd, env.CARGO_TARGET_DIR || 'target');
  fs.mkdirSync(targetDir, { recursive: true });
  for (const name of fs.readdirSync(targetDir).filter((entry) => entry.endsWith('.profraw'))) {
    fs.rmSync(path.join(targetDir, name), { force: true });
  }
  const showEnv = { command: 'cargo', args: ['llvm-cov', 'show-env', '--sh'], cwd: apiCwd };
  const llvmEnv = parseLlvmCovEnvironment(run(showEnv, { env, spawnSyncImpl, capture: true }));
  const instrumentedEnv = { ...env, ...llvmEnv, CARGO_PROFILE_TEST_DEBUG: '0' };
  const fullInventory = JSON.parse(run({
    command: 'cargo', args: ['nextest', 'list', '--package', API_SERVER_PACKAGE, '--message-format', 'json'], cwd: apiCwd,
  }, { env: instrumentedEnv, spawnSyncImpl, capture: true }));
  const inventoryResult = validateShardInventories({ fullInventory, shardInventories: inventories });
  for (const profile of profiles) {
    fs.copyFileSync(path.join(shardRoot, profile), path.join(targetDir, profile));
  }
  run({
    command: 'cargo',
    args: ['llvm-cov', 'report', '--package', API_SERVER_PACKAGE, '--json', '--summary-only', '--output-path', mergedPath],
    cwd: apiCwd,
  }, { env: instrumentedEnv, spawnSyncImpl });
  const mergedSummary = readJson(mergedPath);
  const evidence = {
    sha: [...shas][0],
    inventory: inventoryResult,
    shards: metadata,
    coverage: validateCoverageSummary(mergedSummary),
  };
  if (monolithicPath) {
    evidence.comparison = compareCoverageSummaries(readJson(monolithicPath), mergedSummary);
  }
  fs.writeFileSync(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`, 'utf8');
  fs.mkdirSync(path.dirname(coveragePath), { recursive: true });
  fs.copyFileSync(mergedPath, coveragePath);
}

module.exports = {
  buildApiServerShardCommands,
  collectNextestTestIds,
  compareCoverageSummaries,
  mergeApiServerShadow,
  parseLlvmCovEnvironment,
  parseNextestSummary,
  runApiServerShard,
  validateCoverageSummary,
  validateShardInventories,
  validateShardMetadata,
};
