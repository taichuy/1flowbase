// Capture outside Cargo; --read is deliberately Cargo-free, including failures.
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const JAVASCRIPT = 'runtime-extension-sdk/javascript';
const GENERATOR = fs.realpathSync(__filename);
const sha256 = (bytes) => crypto.createHash('sha256').update(bytes).digest('hex');

function featureName(feature) {
  if (feature !== 'default' && feature !== JAVASCRIPT) {
    throw new Error(`Unsupported metadata feature: ${feature}`);
  }
  return feature === 'default' ? 'default' : 'javascript';
}

function commandArgs(api, feature) {
  featureName(feature);
  return ['metadata', ...(feature === 'default' ? [] : ['--features', feature]),
    '--locked', '--offline', '--format-version', '1', '--manifest-path', path.join(api, 'Cargo.toml')];
}

function sourceIdentity(api) {
  const repo = fs.realpathSync(path.join(api, '..'));
  const result = spawnSync('git', ['-C', repo, 'rev-parse', 'HEAD'], { encoding: 'utf8' });
  if (result.status !== 0 || !/^[a-f0-9]{40,64}\s*$/.test(result.stdout || '')) {
    throw new Error('Cannot establish metadata source SHA');
  }
  return { repoRoot: repo, sourceSha: result.stdout.trim(), generatorSha256: sha256(fs.readFileSync(GENERATOR)) };
}

function localInputPaths(api, metadata) {
  if (fs.realpathSync(metadata.workspace_root) !== api || metadata.workspace_root !== api) {
    throw new Error('Metadata workspace root does not match the real API workspace');
  }
  if (!metadata.resolve || !Array.isArray(metadata.resolve.nodes) || !Array.isArray(metadata.packages)) {
    throw new Error('Capture requires complete Cargo resolved metadata');
  }
  return [...new Set([path.join(api, 'Cargo.toml'), path.join(api, 'Cargo.lock'),
    ...metadata.packages.filter((pkg) => pkg.source === null).map((pkg) => fs.realpathSync(pkg.manifest_path)),
  ])].sort();
}

function assertNoCargo() {
  // Reading /proc avoids launching another Cargo even for an admission probe.
  for (const entry of fs.readdirSync('/proc')) {
    if (!/^\d+$/.test(entry)) continue;
    let comm;
    try { comm = fs.readFileSync(`/proc/${entry}/comm`, 'utf8').trim(); }
    catch (error) { if (error.code === 'ENOENT' || error.code === 'ESRCH') continue; throw error; }
    if (comm === 'cargo') throw new Error(`Capture must run without a parent or other Cargo process (pid ${entry})`);
  }
}

function resourceAdmission(api) {
  assertNoCargo();
  const cpu = () => fs.readFileSync('/proc/stat', 'utf8').split('\n')[0].trim().split(/\s+/).slice(1, 9).map(Number);
  const before = cpu();
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 250);
  const delta = cpu().map((value, index) => value - before[index]);
  const cpuUsed = 1 - (delta[3] + delta[4]) / delta.reduce((sum, value) => sum + value, 0);
  const memory = Object.fromEntries(fs.readFileSync('/proc/meminfo', 'utf8').trim().split('\n')
    .map((line) => [line.split(':')[0], Number(line.split(/\s+/)[1])]));
  const physicalRamUsed = 1 - memory.MemAvailable / memory.MemTotal;
  const disk = fs.statfsSync(api);
  const diskUsed = 1 - disk.bfree / disk.blocks;
  const admission = { cpuUsed, physicalRamUsed, diskUsed, cgroup: fs.readFileSync('/proc/self/cgroup', 'utf8') };
  if (![cpuUsed, physicalRamUsed, diskUsed].every((value) => Number.isFinite(value) && value <= 0.9)) {
    throw new Error(`Capture resource admission refused: ${JSON.stringify(admission)}`);
  }
  assertNoCargo();
  return admission;
}

function inputSnapshot(api, metadata) {
  return localInputPaths(api, metadata).map((file) => ({
    path: file, contentBase64: fs.readFileSync(file).toString('base64'),
  }));
}

function validateInputs(api, metadata, inputs) {
  const paths = localInputPaths(api, metadata);
  if (!Array.isArray(inputs) || inputs.length !== paths.length) throw new Error('Capture input set mismatch');
  paths.forEach((file, index) => {
    const input = inputs[index];
    if (input.path !== file || typeof input.contentBase64 !== 'string') throw new Error('Capture input provenance mismatch');
    const captured = Buffer.from(input.contentBase64, 'base64');
    if (captured.toString('base64') !== input.contentBase64 || !fs.readFileSync(file).equals(captured)) {
      throw new Error(`Stale captured metadata input: ${file}`);
    }
  });
}

function readCapturedMetadata({ apiDir, directory, feature = 'default' }) {
  const api = fs.realpathSync(apiDir);
  const name = featureName(feature);
  const capture = path.resolve(directory);
  const provenanceBytes = fs.readFileSync(path.join(capture, `${name}.receipt.json`));
  const provenance = JSON.parse(provenanceBytes);
  const raw = fs.readFileSync(path.join(capture, `${name}.metadata.json`));
  const identity = sourceIdentity(api);
  if (provenance.version !== 1 || provenance.feature !== feature || provenance.workspaceRoot !== api ||
      provenance.repoRoot !== identity.repoRoot || provenance.sourceSha !== identity.sourceSha ||
      provenance.generatorSha256 !== identity.generatorSha256 ||
      provenance.metadataSha256 !== sha256(raw) || provenance.command?.executable !== 'cargo' ||
      provenance.command?.cwd !== api || provenance.command?.status !== 0 ||
      !Number.isInteger(provenance.command?.pid) || provenance.command.pid <= 0 ||
      !Number.isFinite(Date.parse(provenance.command?.startedAt)) ||
      !Number.isFinite(Date.parse(provenance.command?.finishedAt)) ||
      JSON.stringify(provenance.command?.args) !== JSON.stringify(commandArgs(api, feature))) {
    throw new Error(`Captured metadata provenance mismatch for ${feature}`);
  }
  const metadata = JSON.parse(raw);
  validateInputs(api, metadata, provenance.inputs);
  // Reject a capture replaced while the reader was validating its provenance.
  if (!fs.readFileSync(path.join(capture, `${name}.receipt.json`)).equals(provenanceBytes)) {
    throw new Error('Capture provenance changed during validation');
  }
  return raw;
}

function captureMetadata({ apiDir, directory }) {
  const api = fs.realpathSync(apiDir);
  const identity = sourceIdentity(api);
  assertNoCargo();
  // A new exclusive directory prevents stale or partially overwritten graphs.
  fs.mkdirSync(directory, { recursive: false });
  const receipts = [];
  for (const feature of ['default', JAVASCRIPT]) {
    assertNoCargo();
    // Also compare all inputs from the previous graph before starting Cargo.
    for (const previous of receipts) {
      validateInputs(api, previous.metadata, previous.receipt.inputs);
    }
    const args = commandArgs(api, feature);
    const admission = resourceAdmission(api);
    const startedAt = new Date().toISOString();
    const result = spawnSync('cargo', args, { cwd: api, maxBuffer: 64 * 1024 * 1024 });
    const finishedAt = new Date().toISOString();
    const name = featureName(feature);
    fs.writeFileSync(path.join(directory, `${name}.stderr.log`), result.stderr || Buffer.alloc(0));
    if (result.error || result.status !== 0) {
      throw new Error(`cargo metadata ${feature} failed: ${result.error || result.stderr?.toString()}`);
    }
    const metadata = JSON.parse(result.stdout);
    const receipt = { version: 1, feature, workspaceRoot: api, ...identity,
      metadataSha256: sha256(result.stdout), inputs: inputSnapshot(api, metadata),
      command: { executable: 'cargo', args, cwd: api, pid: result.pid, status: result.status, startedAt, finishedAt, admission } };
    fs.writeFileSync(path.join(directory, `${name}.metadata.json`), result.stdout);
    fs.writeFileSync(path.join(directory, `${name}.receipt.json`), `${JSON.stringify(receipt, null, 2)}\n`);
    receipts.push({ metadata, receipt });
  }
  // Fail capture if source revision or any captured manifest moved mid-command.
  if (JSON.stringify(sourceIdentity(api)) !== JSON.stringify(identity)) throw new Error('Source changed during capture');
  for (const feature of ['default', JAVASCRIPT]) readCapturedMetadata({ apiDir: api, directory, feature });
}

function parseArgs(args) {
  const [mode, ...rest] = args;
  if (mode !== '--capture' && mode !== '--read') throw new Error('Use --capture or --read');
  const values = {};
  for (let i = 0; i < rest.length; i += 2) {
    const key = rest[i];
    if (!['--api-dir', '--directory', '--feature'].includes(key) || !rest[i + 1] || values[key] !== undefined) {
      throw new Error(`Invalid metadata capture argument: ${key}`);
    }
    values[key] = rest[i + 1];
  }
  if (!values['--directory']) throw new Error('--directory is required');
  const feature = values['--feature'] || 'default';
  featureName(feature);
  if (mode === '--capture' && values['--feature']) throw new Error('--capture always captures both feature graphs');
  return { mode, apiDir: values['--api-dir'] || path.resolve(__dirname, '../../../api'),
    directory: values['--directory'], feature };
}

if (require.main === module) {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (options.mode === '--capture') captureMetadata(options);
    else process.stdout.write(readCapturedMetadata(options));
  } catch (error) {
    process.stderr.write(`dependency metadata: ${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = { JAVASCRIPT, commandArgs, parseArgs, captureMetadata, readCapturedMetadata };
