const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { parseCargoTestCounts } = require('../verify/cargo-test-results.js');

function main(_argv = [], deps = {}) {
  const env = deps.env || process.env;
  const repoRoot = deps.repoRoot || path.resolve(__dirname, '../../..');
  const writeStdout = deps.writeStdout || (text => process.stdout.write(text));
  const writeStderr = deps.writeStderr || (text => process.stderr.write(text));
  const result = (deps.spawnSyncImpl || spawnSync)('cargo', [
    'test', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'runtime-extension-host',
    '--test', 'official_plugin_compatibility', '--', '--ignored',
  ], { cwd: repoRoot, env, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  writeStdout(result.stdout || '');
  writeStderr(result.stderr || '');
  if (result.error) writeStderr(`${result.error.message}\n`);
  if (result.status !== 0) return result.status || 1;
  const counts = parseCargoTestCounts(`${result.stdout || ''}\n${result.stderr || ''}`);
  if (!(counts.passedCount > 0 && counts.failedCount === 0)) {
    writeStderr('Official package conformance requires executed passing tests; refusing empty or failed evidence\n');
    return 1;
  }
  return 0;
}

if (require.main === module) process.exitCode = main(process.argv.slice(2));
module.exports = { main };
