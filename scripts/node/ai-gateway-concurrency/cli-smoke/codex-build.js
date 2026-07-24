'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const {
  commandIdentity,
  inspectSourceProvenance,
  sha256File,
} = require('./provenance');

const CODEX_SOURCE_IDENTITY = 'github:openai/codex';
const EVIDENCE_SCHEMA = '1flowbase.codex-low-memory-build/v1';

function defaultRunCommand(executable, args, options = {}) {
  const result = spawnSync(executable, args, {
    cwd: options.cwd,
    env: options.env,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  if (result.error) throw result.error;
  return {
    status: result.status,
    signal: result.signal ?? null,
    stdout: result.stdout ?? '',
    stderr: result.stderr ?? '',
  };
}

function requireSuccess(executable, args, options, runCommand) {
  const result = runCommand(executable, args, options);
  if (result.status !== 0) {
    throw new Error(`${executable} exited with ${result.status}: ${String(result.stderr ?? '').trim()}`);
  }
  return result;
}

function requireVersion(value) {
  const version = String(value).trim();
  if (!/^\d+\.\d+\.\d+$/u.test(version)) throw new Error(`invalid resolved rusty_v8 version: ${version}`);
  return version;
}

function rustcHost(value) {
  const match = /^host:\s*(\S+)$/mu.exec(String(value));
  if (!match) throw new Error('rustc -vV omitted host target');
  return match[1];
}

function planCodexBuild(sourceRoot, outputDir, version, target, jobs) {
  const bindingDir = path.join(outputDir, 'rusty_v8');
  const names = {
    archive: `librusty_v8_release_${target}.a.gz`,
    binding: `src_binding_release_${target}.rs`,
    checksums: `rusty_v8_release_${target}.sha256`,
  };
  const releaseTag = `rusty-v8-v${version}`;
  const baseUrl = `https://github.com/openai/codex/releases/download/${releaseTag}`;
  const paths = Object.fromEntries(Object.entries(names).map(([key, name]) => [key, path.join(bindingDir, name)]));
  return {
    version,
    target,
    releaseTag,
    baseUrl,
    names,
    paths,
    urls: [names.archive, names.binding, names.checksums].map((name) => `${baseUrl}/${name}`),
    environment: {
      RUSTY_V8_ARCHIVE: paths.archive,
      RUSTY_V8_SRC_BINDING_PATH: paths.binding,
    },
    build: {
      executable: 'cargo',
      cwd: sourceRoot,
      args: [
        'build', '--locked', '--manifest-path', 'codex-rs/Cargo.toml',
        '--bin', 'codex', '--jobs', String(jobs),
      ],
    },
    executablePath: path.join(sourceRoot, 'codex-rs', 'target', 'debug', 'codex'),
    evidencePath: path.join(outputDir, 'codex-build-provenance.json'),
  };
}

function parseChecksumManifest(value, target) {
  const lines = String(value).trimEnd().split(/\r?\n/u);
  if (lines.length !== 2) throw new Error('rusty_v8 checksum manifest must contain exactly two lines');
  const rows = lines.map((line) => {
    const match = /^([a-f0-9]{64})\s{2}([^/\\\s]+)$/u.exec(line);
    if (!match) throw new Error('rusty_v8 checksum manifest row is invalid');
    return { sha256: match[1], file: match[2] };
  });
  const expected = [
    `librusty_v8_release_${target}.a.gz`,
    `src_binding_release_${target}.rs`,
  ];
  if (rows.map((row) => row.file).sort().join('\n') !== expected.sort().join('\n')) {
    throw new Error('rusty_v8 checksum manifest must name only the two official rusty_v8 files');
  }
  return rows;
}

function writeEvidence(filePath, value) {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
}

function runCodexBuild(options, dependencies = {}) {
  const sourceRoot = path.resolve(options.sourceRoot);
  const outputDir = path.resolve(options.outputDir);
  const jobs = options.jobs ?? 2;
  if (!Number.isInteger(jobs) || jobs < 1) throw new Error('jobs must be a positive integer');
  const outputRelative = path.relative(sourceRoot, outputDir);
  if (outputRelative === '' || (!outputRelative.startsWith('..') && !path.isAbsolute(outputRelative))) {
    throw new Error('output directory must be outside the clean Codex source worktree');
  }
  const runCommand = dependencies.runCommand || defaultRunCommand;
  const inspectSource = dependencies.inspectSource || ((input) => inspectSourceProvenance('codex', input));
  const source = inspectSource({ sourceRoot, sourceIdentity: CODEX_SOURCE_IDENTITY });
  const versionScript = path.join(sourceRoot, '.github', 'scripts', 'rusty_v8_bazel.py');
  const manifestPath = path.join(sourceRoot, 'codex-rs', 'Cargo.toml');
  for (const [filePath, label] of [[versionScript, 'rusty_v8 version script'], [manifestPath, 'Codex Cargo manifest']]) {
    if (!fs.existsSync(filePath) || !fs.statSync(filePath).isFile()) throw new Error(`${label} does not exist`);
  }
  const version = requireVersion(requireSuccess(
    'python3', [versionScript, 'resolved-v8-crate-version'], { cwd: sourceRoot }, runCommand
  ).stdout);
  const target = rustcHost(requireSuccess('rustc', ['-vV'], { cwd: sourceRoot }, runCommand).stdout);
  const plan = planCodexBuild(sourceRoot, outputDir, version, target, jobs);
  fs.mkdirSync(path.dirname(plan.paths.archive), { recursive: true, mode: 0o700 });
  for (const filePath of Object.values(plan.paths)) fs.rmSync(filePath, { force: true });
  for (const [index, url] of plan.urls.entries()) {
    const output = [plan.paths.archive, plan.paths.binding, plan.paths.checksums][index];
    requireSuccess('curl', [
      '--fail', '--silent', '--show-error', '--location',
      '--retry', '3', '--retry-all-errors', '--output', output, url,
    ], { cwd: outputDir }, runCommand);
  }
  const checksumRows = parseChecksumManifest(fs.readFileSync(plan.paths.checksums, 'utf8'), target);
  requireSuccess('sha256sum', ['-c', plan.paths.checksums], {
    cwd: path.dirname(plan.paths.checksums),
  }, runCommand);
  for (const row of checksumRows) {
    const actual = sha256File(path.join(path.dirname(plan.paths.checksums), row.file));
    if (actual !== row.sha256) throw new Error(`rusty_v8 checksum mismatch for ${row.file}`);
  }
  const buildEnv = { ...process.env, ...plan.environment };
  const buildResult = runCommand(plan.build.executable, plan.build.args, {
    cwd: plan.build.cwd,
    env: buildEnv,
  });
  const baseEvidence = {
    schema_version: EVIDENCE_SCHEMA,
    status: buildResult.status === 0 ? 'pass' : 'fail',
    source: source.source,
    toolchain_and_lockfiles: source.toolchain_and_lockfiles ?? [],
    rusty_v8: {
      version,
      target,
      release_tag: plan.releaseTag,
      urls: plan.urls,
      checksums: checksumRows,
      artifacts: {
        archive: { path: plan.paths.archive, sha256: sha256File(plan.paths.archive) },
        binding: { path: plan.paths.binding, sha256: sha256File(plan.paths.binding) },
        manifest: { path: plan.paths.checksums, sha256: sha256File(plan.paths.checksums) },
      },
    },
    build: {
      profile: 'debug',
      jobs,
      command: commandIdentity([plan.build.executable, ...plan.build.args].join('\u0000')),
      cwd: plan.build.cwd,
      environment: plan.environment,
      exit_code: buildResult.status,
      signal: buildResult.signal ?? null,
    },
  };
  if (buildResult.status !== 0) {
    writeEvidence(plan.evidencePath, baseEvidence);
    throw new Error(`cargo exited with ${buildResult.status}: ${String(buildResult.stderr ?? '').trim()}`);
  }
  if (!fs.existsSync(plan.executablePath) || !fs.statSync(plan.executablePath).isFile()) {
    throw new Error('Codex debug executable was not produced');
  }
  fs.accessSync(plan.executablePath, fs.constants.X_OK);
  const evidence = {
    ...baseEvidence,
    provenance_claim: 'source-built-from-fixed-git-commit',
    executable: { path: plan.executablePath, sha256: sha256File(plan.executablePath) },
  };
  writeEvidence(plan.evidencePath, evidence);
  return evidence;
}

module.exports = {
  CODEX_SOURCE_IDENTITY,
  EVIDENCE_SCHEMA,
  parseChecksumManifest,
  planCodexBuild,
  runCodexBuild,
  rustcHost,
};
