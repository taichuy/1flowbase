'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { ARTIFACT_SCHEMA } = require('./contract');

const SECRET_FIELD = /(^|_)(api_?key|token|secret|credential|authorization|password|private|cookie|csrf|master_?key)($|_)/iu;
const DIAGNOSTIC_FIELD = /(^|_)(error|cleanup|diagnostic|message|stdout|stderr|output|text|body)($|_)/iu;
const SECRET_PATTERN = /(bearer\s+)[^\s"']+|\bsk-[A-Za-z0-9._-]{8,}\b/giu;
const URL_PATTERN = /\b(?:postgres(?:ql)?|https?):\/\/[^\s"'<>]+/giu;
const ENV_SECRET_KEY = /(^|_)(password|secret|token|private|credential)($|_)/iu;
const ENV_API_KEY = /(^|_)api_?key($|_)/iu;
const ENV_API_KEY_ID = /(^|_)api_?key_id($|_)/iu;
const ENV_SECRET_SELECTOR = /(^|_)secret_resolver($|_)/iu;
const MAX_TEXT_BYTES = 128 * 1024;

function normalizedField(key) {
  return String(key).replace(/([a-z0-9])([A-Z])/gu, '$1_$2').toLowerCase();
}

function credentialBearingUrl(value) {
  try {
    const parsed = new URL(value);
    return parsed.username || parsed.password ? value : null;
  } catch { return null; }
}

function classifiedEnvSecret(descriptor) {
  const key = normalizedField(descriptor.key);
  if (key.includes('public_key')) return null;
  if (ENV_API_KEY_ID.test(key) || ENV_SECRET_SELECTOR.test(key)) return null;
  if (key.includes('database_url')) {
    const url = credentialBearingUrl(descriptor.value);
    return url ? { kind: 'credential_url', value: url } : null;
  }
  if (ENV_SECRET_KEY.test(key) || ENV_API_KEY.test(key)) {
    return { kind: 'credential', value: descriptor.value };
  }
  return null;
}

function normalizedDescriptors(secrets) {
  if (Array.isArray(secrets)) {
    return secrets.map((descriptor) => typeof descriptor === 'string'
      ? { kind: credentialBearingUrl(descriptor) ? 'credential_url' : 'credential', value: descriptor }
      : descriptor);
  }
  return [
    ...(secrets?.credentials || []).map((value) => ({ kind: 'credential', value })),
    ...(secrets?.credentialUrls || []).map((value) => ({ kind: 'credential_url', value })),
    ...(secrets?.descriptors || []),
  ];
}

function secretMetadata(secrets) {
  return normalizedDescriptors(secrets).reduce((metadata, descriptor) => {
    if (!descriptor || typeof descriptor.value !== 'string' || !descriptor.value) return metadata;
    const classified = descriptor.kind === 'env' ? classifiedEnvSecret(descriptor) : descriptor;
    if (!classified) return metadata;
    if (classified.kind === 'credential_url') metadata.credentialUrls.push(classified.value);
    if (classified.kind === 'credential') metadata.credentials.push(classified.value);
    return metadata;
  }, { credentials: [], credentialUrls: [] });
}

function exactVariants(input) {
  const raw = String(input);
  const jsonEscaped = JSON.stringify(raw).slice(1, -1);
  const doubleEscaped = JSON.stringify(jsonEscaped).slice(1, -1);
  return [raw, jsonEscaped, doubleEscaped, encodeURIComponent(raw)];
}

function redactCredentialUrl(input) {
  return input.replace(URL_PATTERN, (candidate) => {
    let suffix = '';
    let value = candidate;
    while (/[),.;]$/u.test(value)) {
      suffix = `${value.at(-1)}${suffix}`;
      value = value.slice(0, -1);
    }
    try {
      const parsed = new URL(value);
      if (!parsed.username && !parsed.password) return candidate;
      return `${parsed.protocol}//<redacted>@${parsed.host}${parsed.pathname}${parsed.search}${parsed.hash}${suffix}`;
    } catch { return candidate; }
  });
}

function redactString(input, secrets = [], diagnostic = true) {
  let value = String(input);
  const metadata = secretMetadata(secrets);
  value = redactCredentialUrl(value);
  if (diagnostic) {
    const exactValues = [...new Set(
      [...metadata.credentialUrls, ...metadata.credentials].flatMap(exactVariants),
    )].filter(Boolean).sort((left, right) => right.length - left.length);
    for (const secret of exactValues) value = value.split(secret).join('<redacted>');
    value = value.replace(SECRET_PATTERN, (match, bearer) => bearer ? `${bearer}<redacted>` : '<redacted>');
  }
  const bytes = Buffer.byteLength(value);
  return bytes <= MAX_TEXT_BYTES
    ? value
    : `${Buffer.from(value).subarray(0, MAX_TEXT_BYTES).toString('utf8')}\n<truncated:${bytes - MAX_TEXT_BYTES}-bytes>`;
}

function redact(value, secrets = [], key = '', diagnostic = false) {
  const normalizedKey = normalizedField(key);
  if (SECRET_FIELD.test(normalizedKey) && !normalizedKey.endsWith('_env')) {
    return value ? '<redacted>' : value;
  }
  const inDiagnostic = diagnostic || DIAGNOSTIC_FIELD.test(normalizedKey);
  if (typeof value === 'string') return redactString(value, secrets, inDiagnostic);
  if (Array.isArray(value)) return value.map((item) => redact(item, secrets, key, inDiagnostic));
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value)
      .map(([name, item]) => [name, redact(item, secrets, name, inDiagnostic)]));
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
