const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { walkFiles } = require('../check-rust-backend/core.js');

function apiFormatInputs(repoRoot) {
  if (typeof repoRoot !== 'string' || !path.isAbsolute(repoRoot)) {
    throw new Error('Rust format requires an explicit absolute repository root');
  }
  const manifest = fs.readFileSync(path.join(repoRoot, 'api/Cargo.toml'), 'utf8');
  // This repository uses one literal workspace edition. Reject a changed shape
  // rather than guessing standalone rustfmt's edition or parsing arbitrary TOML.
  const sections = [...manifest.matchAll(/^\[workspace\.package\]\s*\r?\n([\s\S]*?)(?=^\[|$(?![\s\S]))/gm)];
  const editions = sections.length === 1
    ? [...sections[0][1].matchAll(/^edition\s*=\s*"([^"]+)"\s*(?:#.*)?$/gm)]
    : [];
  if (editions.length !== 1 || editions[0][1] !== '2021') {
    throw new Error('Rust format requires the declared workspace.package edition = "2021"');
  }
  const roots = ['src', 'tests'].map((name) => path.join(repoRoot, 'api/apps/api-server', name));
  const files = roots.flatMap((root) => {
    if (!fs.existsSync(root) || !fs.statSync(root).isDirectory()) {
      throw new Error('Missing API Rust format source directory: ' + root);
    }
    const entries = walkFiles(root);
    if (!entries.length) throw new Error('Empty API Rust format source directory: ' + root);
    return entries;
  });
  return { edition: editions[0][1], files: [...new Set(files)].sort() };
}

function runRustFormat({ repoRoot, env = process.env, spawnSyncImpl = spawnSync }) {
  const { edition, files } = apiFormatInputs(repoRoot);
  const commands = [
    ['cargo', ['fmt', '--all', '--', '--check']],
    // Explicit files cover include! roots and test modules inside item macros.
    ['rustfmt', ['--check', '--edition', edition, ...files]],
  ];
  for (const [command, args] of commands) {
    const result = spawnSyncImpl(command, args, {
      cwd: path.join(repoRoot, 'api'), env, stdio: 'inherit',
    });
    if (result.error) throw result.error;
    if (result.status !== 0) return result.status ?? 1;
  }
  return 0;
}

if (require.main === module) {
  try {
    if (process.argv.length !== 3) throw new Error('Rust format requires exactly one repository root');
    process.exitCode = runRustFormat({ repoRoot: process.argv[2] });
  } catch (error) {
    process.stderr.write('[rust-format] ' + error.message + '\n');
    process.exitCode = 1;
  }
}

module.exports = { apiFormatInputs, runRustFormat };
