'use strict';

const SENSITIVE_KEY = /(?:authorization|api[-_]?key|token|credential|secret)/iu;

function redactString(value, secrets) {
  return secrets.filter(Boolean).reduce(
    (current, secret) => current.split(secret).join('<redacted>'),
    value,
  );
}

function redact(value, secrets = []) {
  if (typeof value === 'string') return redactString(value, secrets);
  if (Array.isArray(value)) return value.map((child) => redact(child, secrets));
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(Object.entries(value).map(([key, child]) => [
    key, SENSITIVE_KEY.test(key) ? '<redacted>' : redact(child, secrets),
  ]));
}

module.exports = { redact, redactString };
