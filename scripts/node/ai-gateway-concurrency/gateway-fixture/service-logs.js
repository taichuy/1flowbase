'use strict';

const fs = require('node:fs');
const path = require('node:path');

const SERVICE_LOG_BYTE_CAP = 64 * 1024;
const REDACTED = '[REDACTED]';

function redactServiceLog(value, secrets = []) {
  let sanitized = String(value ?? '');
  const explicit = [...new Set(secrets.filter((secret) => typeof secret === 'string' && secret))]
    .sort((left, right) => right.length - left.length);
  for (const secret of explicit) sanitized = sanitized.replaceAll(secret, REDACTED);
  return sanitized
    .replace(/postgres(?:ql)?:\/\/[^\s"']+/giu, `postgresql://${REDACTED}`)
    .replace(/\b(?:API_DATABASE_URL|BOOTSTRAP_ROOT_PASSWORD|API_PROVIDER_SECRET_MASTER_KEY)=\S+/gu, (match) => `${match.split('=', 1)[0]}=${REDACTED}`)
    .replace(/\bBearer\s+[^\s"']+/giu, `Bearer ${REDACTED}`)
    .replace(/\bsk-[A-Za-z0-9_-]+/gu, REDACTED)
    .replace(/\bgateway_session=[^;\s"']+/gu, `gateway_session=${REDACTED}`)
    .replace(/\bfixture-(?:openai|anthropic)-token\b/gu, REDACTED);
}

function byteTail(value, cap) {
  const bytes = Buffer.from(value, 'utf8');
  if (bytes.length <= cap) return value;
  let tail = bytes.subarray(bytes.length - cap).toString('utf8');
  while (Buffer.byteLength(tail, 'utf8') > cap) tail = tail.slice(1);
  return tail;
}

function serviceLogDocument(service, handle, secrets) {
  const heading = `# ${service}\n\n## stdout\n`;
  const separator = '\n\n## stderr\n';
  const ending = '\n';
  const contentBudget = SERVICE_LOG_BYTE_CAP - Buffer.byteLength(heading + separator + ending);
  const stdoutBudget = Math.floor(contentBudget / 2);
  const stderrBudget = contentBudget - stdoutBudget;
  const stdout = byteTail(redactServiceLog(handle?.stdout?.() ?? '', secrets), stdoutBudget);
  const stderr = byteTail(redactServiceLog(handle?.stderr?.() ?? '', secrets), stderrBudget);
  return `${heading}${stdout}${separator}${stderr}${ending}`;
}

function persistServiceLogs({ artifactRoot, services, secrets = [], fsImpl = fs }) {
  fsImpl.mkdirSync(artifactRoot, { recursive: true });
  const paths = {};
  const errors = [];
  for (const [service, handle] of Object.entries(services)) {
    const filePath = path.join(artifactRoot, `service-${service}.log`);
    try {
      fsImpl.writeFileSync(filePath, serviceLogDocument(service, handle, secrets), { mode: 0o600 });
      paths[service] = filePath;
    } catch (error) {
      errors.push(`${service}: ${error.message}`);
    }
  }
  if (errors.length) throw new Error(`service log persistence failed: ${errors.join('; ')}`);
  return paths;
}

module.exports = {
  REDACTED,
  SERVICE_LOG_BYTE_CAP,
  byteTail,
  persistServiceLogs,
  redactServiceLog,
  serviceLogDocument,
};
