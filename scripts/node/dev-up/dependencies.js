const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { buildServiceEnv } = require('./env.js');
const { runPhase } = require('./phases.js');
const { log } = require('./cli.js');

function dependencyFingerprint(webDir, env = process.env) {
  const hash = crypto.createHash('sha256');
  hash.update(JSON.stringify([process.version, process.platform, process.arch, env.NODE_ENV || '', env.npm_config_production || '']));
  function visit(dir) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      if (['node_modules', '.git', 'dist', 'coverage', '.turbo'].includes(entry.name)) continue;
      const file = path.join(dir, entry.name);
      if (entry.isDirectory()) visit(file);
      else if (entry.isFile() && (['package.json', 'pnpm-lock.yaml', 'pnpm-workspace.yaml', '.npmrc', '.pnpmfile.cjs'].includes(entry.name) || file.startsWith(path.join(webDir, 'patches') + path.sep))) {
        hash.update(path.relative(webDir, file));
        hash.update(fs.readFileSync(file));
      }
    }
  }
  visit(webDir);
  return hash.digest('hex');
}

async function prepareFrontend(service, { runPhaseImpl = runPhase, logImpl = log } = {}) {
  const env = buildServiceEnv(service);
  const receipt = path.join(path.dirname(service.pidFile), 'frontend-dependencies.json');
  const modules = path.join(service.cwd, 'node_modules', '.modules.yaml');
  const installedLock = path.join(service.cwd, 'node_modules', '.pnpm', 'lock.yaml');
  const appModules = path.join(service.cwd, 'app', 'node_modules');
  const fingerprint = dependencyFingerprint(service.cwd, env);
  const installation = () => [modules, installedLock].map((file) => fs.existsSync(file) ? crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex') : null);
  let previous;
  try { previous = JSON.parse(fs.readFileSync(receipt, 'utf8')); } catch (error) {
    if (error.code !== 'ENOENT' && !(error instanceof SyntaxError)) throw error;
  }
  const state = installation();
  if (previous?.fingerprint === fingerprint && state.every((item) => item !== null) && fs.existsSync(appModules) && previous.installation === JSON.stringify(state)) {
    logImpl('frontend dependencies: unchanged; skipping pnpm install');
    return;
  }
  fs.rmSync(receipt, { force: true });
  await runPhaseImpl(service.command, ['install', '--frozen-lockfile'], {
    cwd: service.cwd, env, signal: service.signal,
    logFile: path.join(path.dirname(service.logFile), 'frontend-install.log'),
    label: 'frontend dependencies', timeoutMs: 180_000, inheritStdin: true, logImpl,
  });
  // Record post-install inputs because package hooks may update inputs.
  const installed = installation();
  if (installed.every((item) => item !== null) && fs.existsSync(appModules)) {
    fs.writeFileSync(receipt, JSON.stringify({ fingerprint: dependencyFingerprint(service.cwd, env), installation: JSON.stringify(installed) }));
  }
}

module.exports = { dependencyFingerprint, prepareFrontend };
