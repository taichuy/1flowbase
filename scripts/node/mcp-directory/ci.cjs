const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');

const out = path.resolve('tmp/test-governance/mcp-directory-ci');
fs.mkdirSync(out, { recursive: true });
const candidate = spawnSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' });
if (candidate.status !== 0) throw new Error('Cannot resolve candidate');
fs.writeFileSync(path.join(out, 'candidate-sha.txt'), candidate.stdout.trim() + '\n');
const env = { ...process.env, CARGO_BUILD_JOBS: String(os.availableParallelism()) };
const receipts = [];
function run(name, args, tests = true) {
  const file = path.join(out, `${name}.log`);
  const fd = fs.openSync(file, 'w');
  const start = Date.now();
  let result;
  try { result = spawnSync('cargo', args, { env, stdio: ['ignore', fd, fd] }); }
  finally { fs.closeSync(fd); }
  const log = fs.readFileSync(file, 'utf8');
  const matches = [...log.matchAll(/test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored/g)];
  const counts = matches.reduce((sum, match) => sum.map((n, i) => n + Number(match[i + 1])), [0, 0, 0]);
  const ok = result.status === 0 && (!tests || (counts[0] > 0 && counts[1] === 0 && counts[2] === 0));
  receipts.push({ name, command: ['cargo', ...args], exit_code: result.status, passed: counts[0], failed: counts[1], ignored: counts[2], duration_ms: Date.now() - start, ok });
  console.log(JSON.stringify(receipts.at(-1)));
  if (!ok) console.error(log.slice(-6000));
  fs.writeFileSync(path.join(out, 'receipt.json'), JSON.stringify(receipts, null, 2));
  return ok;
}
const base = ['test', '--locked', '--manifest-path', 'api/Cargo.toml'];
run('contracts', [...base, '-p', 'control-plane-contracts']);
run('permissions', [...base, '-p', 'access-control', '--lib']);
run('postgres-directory', [...base, '-p', 'control-plane-postgres-tests', '--test', 'mcp_management_integration', 'directory_', '--', '--nocapture']);
run('api-directory', [...base, '-p', 'api-server', '--test', 'api_mcp_management', 'directory_', '--', '--nocapture']);
run('interface-inventory', [...base, '-p', 'api-server', '--test', 'interface_coverage_inventory']);
if (run('runtime-build', ['build', '--locked', '--manifest-path', 'api/Cargo.toml', '--workspace', '--bin', 'api-server'], false)) {
  const binary = path.join(out, 'api-server');
  const stripped = spawnSync('strip', ['-o', binary, 'api/target/debug/api-server'], { encoding: 'utf8' });
  if (stripped.status !== 0) throw new Error(stripped.stderr);
  const hash = crypto.createHash('sha256');
  const fd = fs.openSync(binary, 'r');
  const buffer = Buffer.alloc(64 * 1024);
  try { let count; while ((count = fs.readSync(fd, buffer, 0, buffer.length, null)) > 0) hash.update(buffer.subarray(0, count)); }
  finally { fs.closeSync(fd); }
  fs.writeFileSync(path.join(out, 'runtime-sha256.txt'), hash.digest('hex') + '\n');
}
process.exitCode = receipts.every(receipt => receipt.ok) ? 0 : 1;
