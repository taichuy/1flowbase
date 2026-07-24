'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

function redact(text, secrets) {
  return secrets.filter(Boolean).reduce(
    (value, secret) => value.split(secret).join('<redacted-application-key>'),
    text
  );
}

function writeJson(filePath, value) {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
}

function manifestDigest(filePath) {
  return crypto.createHash('sha256').update(fs.readFileSync(filePath)).digest('hex');
}

function prepareEvidenceRoot(outputRoot) {
  fs.mkdirSync(outputRoot, { recursive: true });
  for (const name of ['config-manifest.json', 'codex.json', 'claude.json', 'opencode.json']) {
    fs.rmSync(path.join(outputRoot, name), { force: true });
  }
  for (const name of ['codex', 'claude', 'opencode']) {
    fs.rmSync(path.join(outputRoot, name), { recursive: true, force: true });
  }
  return outputRoot;
}

function writeConfigManifest(outputRoot, value, secrets = []) {
  const serialized = redact(`${JSON.stringify(value, null, 2)}\n`, secrets);
  fs.writeFileSync(path.join(outputRoot, 'config-manifest.json'), serialized, { mode: 0o600 });
}

function writeClientEvidence(outputRoot, client, result, secrets) {
  const evidence = {
    schema_version: '1flowbase.ai-gateway-cli-smoke-evidence/v1',
    client,
    started_at: result.started_at,
    finished_at: result.finished_at,
    duration_ms: result.duration_ms,
    exit_code: result.exit_code,
    signal: result.signal,
    timed_out: result.timed_out,
    stdout_bytes: result.stdout.bytes,
    stderr_bytes: result.stderr.bytes,
    stdout_overflow: result.stdout.overflow,
    stderr_overflow: result.stderr.overflow,
    stdout: redact(result.stdout.text, secrets),
    stderr: redact(result.stderr.text, secrets),
    pty_markers: result.pty?.markers ?? null,
    pty_observation: result.pty?.observation ?? null,
    timeline_events: result.pty?.timeline_events ?? null,
  };
  writeJson(path.join(outputRoot, `${client}.json`), evidence);
  if (result.pty) {
    const clientRoot = path.join(outputRoot, client);
    fs.mkdirSync(clientRoot, { recursive: true, mode: 0o700 });
    fs.writeFileSync(path.join(clientRoot, 'pty.log'), redact(result.stdout.text, secrets), { mode: 0o600 });
    fs.writeFileSync(path.join(clientRoot, 'timing.log'), result.pty.timing, { mode: 0o600 });
    writeJson(path.join(clientRoot, 'final.json'), evidence);
  }
}

module.exports = {
  manifestDigest,
  prepareEvidenceRoot,
  redact,
  writeClientEvidence,
  writeConfigManifest,
};
