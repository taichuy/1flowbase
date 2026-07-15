const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const DEFAULT_COMPILED_INVENTORY_PATH = path.join(
  'tmp',
  'test-governance',
  'console-operation-compiled-inventory.json'
);
const DEFAULT_BASELINE_INVENTORY_PATH = path.join(
  'scripts',
  'node',
  'console-operation-registry-hygiene',
  'compiled-inventory-baseline.json'
);
const RUN_COMMAND_MAX_BUFFER_BYTES = 16 * 1024 * 1024;

function generateCompiledInventorySnapshot({
  repoRoot,
  env = process.env,
  outputPath = DEFAULT_COMPILED_INVENTORY_PATH,
  spawnSyncImpl = spawnSync,
} = {}) {
  const absoluteOutputPath = path.isAbsolute(outputPath)
    ? outputPath
    : path.join(repoRoot, outputPath);
  fs.mkdirSync(path.dirname(absoluteOutputPath), { recursive: true });
  const result = spawnSyncImpl('cargo', [
    'run',
    '--quiet',
    '-p',
    'api-server',
    '--bin',
    'console_operation_inventory',
    '--',
    absoluteOutputPath,
  ], {
    cwd: path.join(repoRoot, 'api'),
    env: {
      ...env,
      CARGO_INCREMENTAL: '0',
    },
    encoding: 'utf8',
    maxBuffer: RUN_COMMAND_MAX_BUFFER_BYTES,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  if ((result?.status ?? 1) !== 0) {
    throw new Error(
      `Rust compiled inventory generator failed (exit ${result?.status ?? 1}): ${result?.stderr || result?.stdout || 'no output'}`
    );
  }
  if (!fs.existsSync(absoluteOutputPath)) {
    throw new Error(`Rust compiled inventory generator did not write ${outputPath}`);
  }
  return path.relative(repoRoot, absoluteOutputPath).split(path.sep).join('/');
}

module.exports = {
  DEFAULT_BASELINE_INVENTORY_PATH,
  DEFAULT_COMPILED_INVENTORY_PATH,
  generateCompiledInventorySnapshot,
};
