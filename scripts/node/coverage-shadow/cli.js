#!/usr/bin/env node

const os = require('node:os');
const path = require('node:path');
const {
  buildApiServerCoverageArchive,
  mergeApiServerShadow,
  runApiServerShard,
} = require('./core.js');

function main(argv = process.argv.slice(2), env = process.env) {
  const [operation, packageName, indexOrCount, maybeCount, monolithicShaPath] = argv;
  if (packageName !== 'api-server') throw new Error(`unsupported coverage shadow package: ${packageName || ''}`);
  const repoRoot = path.resolve(__dirname, '..', '..', '..');
  const archiveDir = env.ONEFLOWBASE_COVERAGE_ARCHIVE_DIR
    ? path.resolve(repoRoot, env.ONEFLOWBASE_COVERAGE_ARCHIVE_DIR)
    : undefined;
  if (operation === 'build-archive') {
    if (!archiveDir) throw new Error('build-archive requires ONEFLOWBASE_COVERAGE_ARCHIVE_DIR');
    buildApiServerCoverageArchive({ repoRoot, archiveDir, env });
    return;
  }
  if (operation === 'shard') {
    runApiServerShard({
      repoRoot,
      shardIndex: Number(indexOrCount),
      shardCount: Number(maybeCount),
      cargoTestThreads: Number(env.CARGO_TEST_THREADS || os.availableParallelism()),
      archiveDir,
      env,
    });
    return;
  }
  if (operation === 'merge') {
    mergeApiServerShadow({
      repoRoot,
      shardCount: Number(indexOrCount),
      monolithicPath: maybeCount ? path.resolve(repoRoot, maybeCount) : undefined,
      monolithicShaPath: monolithicShaPath ? path.resolve(repoRoot, monolithicShaPath) : undefined,
      archiveDir,
      env,
    });
    return;
  }
  throw new Error(`unsupported coverage shadow operation: ${operation || ''}`);
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`[coverage-shadow] ${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = { main };
