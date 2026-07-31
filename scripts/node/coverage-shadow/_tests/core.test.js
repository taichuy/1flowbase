const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  buildApiServerShardCommands,
  collectNextestTestIds,
  compareCoverageSummaries,
  mergeApiServerShadow,
  parseLlvmCovEnvironment,
  runApiServerShard,
  validateShardInventories,
} = require('../core.js');

function inventory(testIds) {
  return {
    'rust-suites': {
      'api-server::lib': {
        testcases: Object.fromEntries(testIds.map((testId) => [testId, { ignored: false }])),
      },
    },
  };
}

test('API coverage shadow shard uses the stable nextest hash partition', () => {
  const commands = buildApiServerShardCommands({
    repoRoot: '/repo',
    shardIndex: 2,
    shardCount: 4,
    cargoTestThreads: 4,
  });

  assert.deepEqual(commands.map((command) => command.args), [
    ['llvm-cov', 'show-env', '--sh'],
    ['nextest', 'list', '--package', 'api-server', '--partition', 'hash:2/4', '--message-format', 'json'],
    ['nextest', 'run', '--package', 'api-server', '--partition', 'hash:2/4', '--test-threads', '4', '--no-fail-fast', '--no-tests=fail'],
  ]);
  assert.match(commands[2].profilePattern, /coverage-shadow\/api-server\/shard-2\/shard-2-%p-%m\.profraw$/u);
});

test('llvm-cov environment parser accepts Cargo ANSI color output', () => {
  assert.deepEqual(parseLlvmCovEnvironment(
    "\u001b[1mexport RUSTC_WRAPPER='/bin/cov'\u001b[0m\nexport CARGO_LLVM_COV=1\n"
  ), {
    RUSTC_WRAPPER: '/bin/cov',
    CARGO_LLVM_COV: '1',
  });
});

test('nextest inventory extraction creates stable binary-qualified test ids', () => {
  const value = inventory(['beta', 'alpha', 'filtered']);
  value['rust-suites']['api-server::lib'].testcases.filtered['filter-match'] = {
    status: 'mismatch',
    reason: 'partition',
  };
  assert.deepEqual(collectNextestTestIds(value), [
    'api-server::lib::alpha',
    'api-server::lib::beta',
  ]);
});

test('four shard inventories must be an exact disjoint union of the full inventory', () => {
  assert.deepEqual(validateShardInventories({
    fullInventory: inventory(['a', 'b', 'c', 'd']),
    shardInventories: [inventory(['a']), inventory(['b']), inventory(['c']), inventory(['d'])],
  }), {
    fullCount: 4,
    shardCount: 4,
  });

  assert.throws(() => validateShardInventories({
    fullInventory: inventory(['a', 'b']),
    shardInventories: [inventory(['a']), inventory(['a']), inventory([]), inventory([])],
  }), /duplicate.*missing/u);
});

test('merged coverage preserves enforced totals and records scheduling-only region drift', () => {
  const summary = {
    data: [{
      totals: {
        lines: { count: 10, covered: 8, percent: 80 },
        functions: { count: 4, covered: 3, percent: 75 },
        regions: { count: 12, covered: 9, percent: 75 },
      },
      files: [{
        filename: '/repo/api/apps/api-server/src/lib.rs',
        summary: { lines: { count: 10, covered: 8, percent: 80 } },
      }],
    }],
  };

  assert.deepEqual(compareCoverageSummaries(summary, structuredClone(summary)), {
    fileCount: 1,
    metrics: ['functions', 'lines'],
    nondeterministicFiles: 0,
    regionCoveredDelta: 0,
  });

  const changed = structuredClone(summary);
  changed.data[0].totals.lines.covered = 7;
  assert.throws(() => compareCoverageSummaries(summary, changed), /mismatch.*lines totals/u);

  const regionSchedulingDifference = structuredClone(summary);
  regionSchedulingDifference.data[0].totals.regions.covered = 8;
  regionSchedulingDifference.data[0].totals.regions.percent = 66.67;
  assert.deepEqual(compareCoverageSummaries(summary, regionSchedulingDifference), {
    fileCount: 1,
    metrics: ['functions', 'lines'],
    nondeterministicFiles: 0,
    regionCoveredDelta: -1,
  });
});

test('shard orchestration fails closed when nextest execution fails', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coverage-shadow-shard-'));
  fs.mkdirSync(path.join(repoRoot, 'api'), { recursive: true });
  let call = 0;
  assert.throws(() => runApiServerShard({
    repoRoot,
    shardIndex: 1,
    shardCount: 4,
    cargoTestThreads: 2,
    env: { QUALITY_GATE_TARGET_SHA: 'abc' },
    spawnSyncImpl() {
      call += 1;
      if (call === 1) {
        return { status: 0, stdout: "export RUSTC_WRAPPER='/bin/cov'\nexport CARGO_LLVM_COV='1'\n" };
      }
      if (call === 2) return { status: 0, stdout: JSON.stringify(inventory(['a'])) };
      return { status: 7, stdout: '', stderr: 'failed' };
    },
  }), /failed with exit code 7/u);
});

test('merge fails closed when a shard artifact is absent', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coverage-shadow-merge-missing-'));
  fs.mkdirSync(path.join(repoRoot, 'tmp/test-governance/coverage-shadow/api-server/downloaded'), { recursive: true });
  assert.throws(() => mergeApiServerShadow({
    repoRoot,
    shardCount: 4,
    monolithicPath: path.join(repoRoot, 'monolithic.json'),
    monolithicShaPath: path.join(repoRoot, 'target-sha.txt'),
    env: { QUALITY_GATE_TARGET_SHA: 'abc' },
  }), /inventory-1\.json/u);
});

test('merge rejects shard and monolithic artifacts from a different SHA before reporting', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coverage-shadow-merge-sha-'));
  const shardRoot = path.join(repoRoot, 'tmp/test-governance/coverage-shadow/api-server/downloaded');
  fs.mkdirSync(shardRoot, { recursive: true });
  for (let index = 1; index <= 4; index += 1) {
    fs.writeFileSync(path.join(shardRoot, `inventory-${index}.json`), JSON.stringify(inventory([String(index)])));
    fs.writeFileSync(path.join(shardRoot, `metadata-${index}.json`), JSON.stringify({ sha: 'old-sha' }));
    fs.writeFileSync(path.join(shardRoot, `shard-${index}-1.profraw`), 'profile');
  }
  const monolithicShaPath = path.join(repoRoot, 'target-sha.txt');
  fs.writeFileSync(monolithicShaPath, 'old-sha\n');
  assert.throws(() => mergeApiServerShadow({
    repoRoot,
    shardCount: 4,
    monolithicPath: path.join(repoRoot, 'monolithic.json'),
    monolithicShaPath,
    env: { QUALITY_GATE_TARGET_SHA: 'new-sha' },
  }), /same frozen SHA/u);
});

test('merge gathers all profiles and writes exact equivalence evidence', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'coverage-shadow-merge-ok-'));
  const shardRoot = path.join(repoRoot, 'tmp/test-governance/coverage-shadow/api-server/downloaded');
  const targetDir = path.join(repoRoot, 'tmp/coverage-target');
  fs.mkdirSync(shardRoot, { recursive: true });
  fs.mkdirSync(path.join(repoRoot, 'api'), { recursive: true });
  for (let index = 1; index <= 4; index += 1) {
    fs.writeFileSync(path.join(shardRoot, `inventory-${index}.json`), JSON.stringify(inventory([String(index)])));
    fs.writeFileSync(path.join(shardRoot, `metadata-${index}.json`), JSON.stringify({ sha: 'frozen-sha' }));
    fs.writeFileSync(path.join(shardRoot, `shard-${index}-1.profraw`), `profile-${index}`);
  }
  const summary = {
    data: [{
      totals: { lines: { count: 1, covered: 1 }, functions: { count: 1, covered: 1 }, regions: { count: 1, covered: 1 } },
      files: [{ filename: '/repo/lib.rs', summary: { lines: { count: 1, covered: 1 } } }],
    }],
  };
  const monolithicPath = path.join(repoRoot, 'monolithic.json');
  const monolithicShaPath = path.join(repoRoot, 'target-sha.txt');
  fs.writeFileSync(monolithicPath, JSON.stringify(summary));
  fs.writeFileSync(monolithicShaPath, 'frozen-sha\n');
  let call = 0;
  mergeApiServerShadow({
    repoRoot,
    shardCount: 4,
    monolithicPath,
    monolithicShaPath,
    env: { QUALITY_GATE_TARGET_SHA: 'frozen-sha', CARGO_TARGET_DIR: targetDir },
    spawnSyncImpl(_command, args) {
      call += 1;
      if (call === 1) {
        return { status: 0, stdout: "export RUSTC_WRAPPER='/bin/cov'\nexport CARGO_LLVM_COV='1'\n" };
      }
      if (call === 2) return { status: 0, stdout: JSON.stringify(inventory(['1', '2', '3', '4'])) };
      const outputPath = args.at(-1);
      fs.writeFileSync(outputPath, JSON.stringify(summary));
      return { status: 0, stdout: '' };
    },
  });
  assert.deepEqual(
    fs.readdirSync(targetDir).filter((name) => name.endsWith('.profraw')).sort(),
    ['shard-1-1.profraw', 'shard-2-1.profraw', 'shard-3-1.profraw', 'shard-4-1.profraw']
  );
  const evidence = JSON.parse(fs.readFileSync(
    path.join(repoRoot, 'tmp/test-governance/coverage-shadow/api-server/equivalence.json'),
    'utf8'
  ));
  assert.equal(evidence.sha, 'frozen-sha');
  assert.equal(evidence.inventory.fullCount, 4);
});
