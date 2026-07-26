'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { ARTIFACT_SCHEMA } = require('./contract');

const SECRET_FIELD = /(^|_)(api_?key|token|secret|credential|authorization)($|_)/iu;
const SECRET_PATTERN = /(bearer\s+)[^\s"']+|\bsk-[A-Za-z0-9._-]{8,}\b/giu;
const MAX_TEXT_BYTES = 128 * 1024;

function redactString(input, secrets = []) {
  let value = String(input);
  for (const secret of secrets.filter(Boolean).sort((left, right) => right.length - left.length)) {
    value = value.split(secret).join('<redacted>');
  }
  value = value.replace(SECRET_PATTERN, (match, bearer) => bearer ? `${bearer}<redacted>` : '<redacted>');
  const bytes = Buffer.byteLength(value);
  return bytes <= MAX_TEXT_BYTES
    ? value
    : `${Buffer.from(value).subarray(0, MAX_TEXT_BYTES).toString('utf8')}\n<truncated:${bytes - MAX_TEXT_BYTES}-bytes>`;
}

function redact(value, secrets = [], key = '') {
  if (SECRET_FIELD.test(key) && !key.endsWith('_env')) return value ? '<redacted>' : value;
  if (typeof value === 'string') return redactString(value, secrets);
  if (Array.isArray(value)) return value.map((item) => redact(item, secrets));
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([name, item]) => [name, redact(item, secrets, name)]));
  }
  return value;
}

function validateArtifact(artifact) {
  if (artifact?.schema_version !== ARTIFACT_SCHEMA) throw new Error('local client artifact schema mismatch');
  if (!['pass', 'fail', 'partial', 'skipped'].includes(artifact.status)) throw new Error('artifact status is invalid');
  if (!Array.isArray(artifact.clients)) throw new Error('artifact clients must be an array');
  if (!artifact.cleanup || !['pass', 'fail'].includes(artifact.cleanup.status)) {
    throw new Error('artifact cleanup status is invalid');
  }
  for (const client of artifact.clients) {
    if (!client.name || !['pass', 'fail', 'skipped'].includes(client.status)) {
      throw new Error('artifact client result is invalid');
    }
    if (!Array.isArray(client.timeline)) throw new Error('artifact client timeline must be an array');
  }
  return artifact;
}

function writeArtifact(filePath, artifact, secrets = []) {
  const safe = validateArtifact(redact(artifact, secrets));
  fs.mkdirSync(path.dirname(filePath), { recursive: true, mode: 0o700 });
  fs.writeFileSync(filePath, `${JSON.stringify(safe, null, 2)}\n`, { mode: 0o600 });
  return safe;
}

function createTimeline(now = () => process.hrtime.bigint()) {
  const start = now();
  const events = [];
  return {
    append(event, detail = {}) {
      events.push({ sequence: events.length + 1, elapsed_ns: String(now() - start), event, ...detail });
    },
    snapshot() { return events.map((event) => ({ ...event })); },
  };
}

module.exports = { MAX_TEXT_BYTES, createTimeline, redact, redactString, validateArtifact, writeArtifact };
