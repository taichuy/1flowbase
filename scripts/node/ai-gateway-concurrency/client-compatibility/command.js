'use strict';

const { execFile } = require('node:child_process');
const path = require('node:path');

function runClientCompatibilityCommand(options, dependencies = {}) {
  const execute = dependencies.execFile ?? execFile;
  const args = [
    path.join(__dirname, 'cli.js'), 'run',
    '--ready-manifest', options.readyManifest,
    '--runtime-root', options.runtimeRoot,
    '--evidence-root', options.evidenceRoot,
  ];
  if (options.lockPath) args.push('--lock', options.lockPath);
  return new Promise((resolve, reject) => {
    execute(process.execPath, args, { encoding: 'utf8', maxBuffer: 1024 * 1024 }, (error, stdout) => {
      if (error) return reject(new Error(`portable ACP compatibility gate failed with exit code ${error.code ?? 'unknown'}; inspect redacted evidence`));
      try { resolve(JSON.parse(stdout)); } catch { reject(new Error('portable ACP compatibility gate emitted invalid JSON')); }
    });
  });
}

module.exports = { runClientCompatibilityCommand };
