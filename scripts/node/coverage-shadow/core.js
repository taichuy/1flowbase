const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const API_SERVER_PACKAGE = 'api-server';
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
      profilePattern: path.join(shardDir, `shard-${shardIndex}-%p-%m.profraw`),
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

function collectNextestTestIds(inventory) {
  const suites = inventory?.['rust-suites'];
  if (!suites || typeof suites !== 'object') throw new Error('invalid nextest JSON inventory');
  return Object.entries(suites)
    .flatMap(([suiteId, suite]) => Object.entries(suite?.testcases || {})
      .filter(([, testcase]) => testcase?.['filter-match']?.status !== 'mismatch')
      .map(([testId]) => `${suiteId}::${testId}`))
    .sort();
}

function validateShardInventories({ fullInventory, shardInventories }) {
  const fullIds = collectNextestTestIds(fullInventory);
  const seen = new Set();
  const duplicates = [];
  for (const shardInventory of shardInventories) {
    for (const testId of collectNextestTestIds(shardInventory)) {
      if (seen.has(testId)) duplicates.push(testId);
      seen.add(testId);
    }
  }
  const missing = fullIds.filter((testId) => !seen.has(testId));
  const extra = [...seen].filter((testId) => !fullIds.includes(testId));
  if (duplicates.length || missing.length || extra.length) {
    throw new Error(`coverage shard inventory mismatch: duplicate=${duplicates.length}, missing=${missing.length}, extra=${extra.length}`);
  }
  return { fullCount: fullIds.length, shardCount: shardInventories.length };
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

function compareCoverageSummaries(monolithic, merged) {
  const expected = canonicalCoverageSummary(monolithic);
  const actual = canonicalCoverageSummary(merged);
  const enforcedMetrics = ['functions', 'lines'];
  for (const metric of enforcedMetrics) {
    const left = expected.totals[metric];
    const right = actual.totals[metric];
    if (left?.count !== right?.count || left?.covered !== right?.covered) {
      throw new Error(`coverage shadow mismatch in enforced ${metric} totals`);
    }
  }
  const structuralMetrics = Object.keys(expected.totals);
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
  return {
    fileCount: expected.files.length,
    metrics: enforcedMetrics,
    nondeterministicFiles,
    regionCoveredDelta: (actual.totals.regions?.covered ?? 0) - (expected.totals.regions?.covered ?? 0),
  };
}

function run(command, { env, spawnSyncImpl = spawnSync, capture = false } = {}) {
  const result = spawnSyncImpl(command.command, command.args, {
    cwd: command.cwd,
    env,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    stdio: capture ? 'pipe' : 'inherit',
  });
  if (result.status !== 0) throw new Error(`${command.command} ${command.args.join(' ')} failed with exit code ${result.status ?? 1}`);
  return result.stdout || '';
}

function runApiServerShard({ repoRoot, shardIndex, shardCount, cargoTestThreads, env = process.env, spawnSyncImpl }) {
  const commands = buildApiServerShardCommands({ repoRoot, shardIndex, shardCount, cargoTestThreads });
  const shardDir = path.dirname(commands[2].profilePattern);
  fs.rmSync(shardDir, { recursive: true, force: true });
  fs.mkdirSync(shardDir, { recursive: true });
  const llvmEnv = parseLlvmCovEnvironment(run(commands[0], { env, spawnSyncImpl, capture: true }));
  const instrumentedEnv = { ...env, ...llvmEnv, LLVM_PROFILE_FILE: commands[2].profilePattern, CARGO_PROFILE_TEST_DEBUG: '0' };
  const inventoryText = run(commands[1], { env: instrumentedEnv, spawnSyncImpl, capture: true });
  const inventory = JSON.parse(inventoryText);
  fs.writeFileSync(path.join(shardDir, `inventory-${shardIndex}.json`), `${JSON.stringify(inventory)}\n`, 'utf8');
  run(commands[2], { env: instrumentedEnv, spawnSyncImpl });
  const profiles = fs.readdirSync(shardDir).filter((name) => name.endsWith('.profraw'));
  if (profiles.length === 0) throw new Error(`coverage shard ${shardIndex}/${shardCount} produced no profraw files`);
  fs.writeFileSync(path.join(shardDir, `metadata-${shardIndex}.json`), `${JSON.stringify({
    package: API_SERVER_PACKAGE,
    partition: `hash:${shardIndex}/${shardCount}`,
    sha: env.QUALITY_GATE_TARGET_SHA || '',
    testCount: collectNextestTestIds(inventory).length,
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
  const root = path.join(repoRoot, 'tmp', 'test-governance', 'coverage-shadow', 'api-server');
  const shardRoot = path.join(root, 'downloaded');
  const inventories = [];
  const metadata = [];
  for (let index = 1; index <= shardCount; index += 1) {
    inventories.push(readJson(path.join(shardRoot, `inventory-${index}.json`)));
    metadata.push(readJson(path.join(shardRoot, `metadata-${index}.json`)));
  }
  const shas = new Set(metadata.map((item) => item.sha));
  const expectedSha = env.QUALITY_GATE_TARGET_SHA || '';
  const monolithicSha = fs.readFileSync(monolithicShaPath, 'utf8').trim();
  if (shas.size !== 1 || !shas.has(expectedSha) || monolithicSha !== expectedSha) {
    throw new Error('coverage shards do not belong to the same frozen SHA');
  }
  const profiles = fs.readdirSync(shardRoot).filter((name) => name.endsWith('.profraw'));
  if (profiles.length < shardCount) throw new Error(`coverage shadow requires all ${shardCount} shard profiles`);

  const apiCwd = path.join(repoRoot, 'api');
  const targetDir = path.resolve(apiCwd, env.CARGO_TARGET_DIR || 'target');
  fs.mkdirSync(targetDir, { recursive: true });
  for (const name of fs.readdirSync(targetDir).filter((entry) => entry.endsWith('.profraw'))) {
    fs.rmSync(path.join(targetDir, name), { force: true });
  }
  for (const profile of profiles) {
    fs.copyFileSync(path.join(shardRoot, profile), path.join(targetDir, profile));
  }
  const showEnv = { command: 'cargo', args: ['llvm-cov', 'show-env', '--sh'], cwd: apiCwd };
  const llvmEnv = parseLlvmCovEnvironment(run(showEnv, { env, spawnSyncImpl, capture: true }));
  const instrumentedEnv = { ...env, ...llvmEnv, CARGO_PROFILE_TEST_DEBUG: '0' };
  const fullInventory = JSON.parse(run({
    command: 'cargo', args: ['nextest', 'list', '--package', API_SERVER_PACKAGE, '--message-format', 'json'], cwd: apiCwd,
  }, { env: instrumentedEnv, spawnSyncImpl, capture: true }));
  const inventoryResult = validateShardInventories({ fullInventory, shardInventories: inventories });
  const mergedPath = path.join(root, 'api-server-merged.json');
  run({
    command: 'cargo',
    args: ['llvm-cov', 'report', '--package', API_SERVER_PACKAGE, '--json', '--summary-only', '--output-path', mergedPath],
    cwd: apiCwd,
  }, { env: instrumentedEnv, spawnSyncImpl });
  const comparison = compareCoverageSummaries(readJson(monolithicPath), readJson(mergedPath));
  fs.writeFileSync(path.join(root, 'equivalence.json'), `${JSON.stringify({
    sha: [...shas][0], inventory: inventoryResult, comparison,
  }, null, 2)}\n`, 'utf8');
}

module.exports = {
  buildApiServerShardCommands,
  collectNextestTestIds,
  compareCoverageSummaries,
  mergeApiServerShadow,
  parseLlvmCovEnvironment,
  runApiServerShard,
  validateShardInventories,
};
