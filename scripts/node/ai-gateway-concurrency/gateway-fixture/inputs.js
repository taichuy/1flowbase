'use strict';

const fs = require('node:fs');
const path = require('node:path');

class GatewayFixtureError extends Error {
  constructor(message) {
    super(message);
    this.name = 'GatewayFixtureError';
  }
}

function fixtureError(message) {
  throw new GatewayFixtureError(message);
}

function requireString(value, label) {
  if (typeof value !== 'string' || value.trim() === '') {
    fixtureError(`${label} is required`);
  }
  return value.trim();
}

function requireFile(value, label, { executable = false } = {}) {
  const resolved = path.resolve(requireString(value, label));
  let stat;
  try {
    stat = fs.statSync(resolved);
  } catch {
    fixtureError(`${label} does not exist`);
  }
  if (!stat.isFile()) {
    fixtureError(`${label} must be a file`);
  }
  if (executable) {
    try {
      fs.accessSync(resolved, fs.constants.X_OK);
    } catch {
      fixtureError(`${label} must be executable`);
    }
  }
  return resolved;
}

function requirePostgresUrl(value) {
  const raw = requireString(value, 'temporary PostgreSQL URL');
  let parsed;
  try {
    parsed = new URL(raw);
  } catch {
    fixtureError('temporary PostgreSQL URL is invalid');
  }
  if (!['postgres:', 'postgresql:'].includes(parsed.protocol) || !parsed.hostname || !parsed.pathname.slice(1)) {
    fixtureError('temporary PostgreSQL URL must name a PostgreSQL host and database');
  }
  const databaseName = decodeURIComponent(parsed.pathname.slice(1)).toLowerCase();
  if (!['qadb', 'fixture', 'test', 'tmp', 'temp'].some((prefix) => databaseName.startsWith(prefix))) {
    fixtureError('temporary PostgreSQL URL must name a disposable PostgreSQL database');
  }
  return raw;
}

function requireLoopbackUrl(value) {
  const raw = requireString(value, 'mock upstream loopback URL');
  let parsed;
  try {
    parsed = new URL(raw);
  } catch {
    fixtureError('mock upstream loopback URL is invalid');
  }
  const host = parsed.hostname.toLowerCase();
  if (parsed.protocol !== 'http:' || !['127.0.0.1', '::1', 'localhost'].includes(host)) {
    fixtureError('mock upstream URL must be plain HTTP on loopback');
  }
  if (parsed.username || parsed.password || parsed.search || parsed.hash) {
    fixtureError('mock upstream URL must not contain credentials, query, or fragment');
  }
  if (parsed.pathname !== '/') {
    fixtureError('mock upstream URL must be the WP1 server base URL without a path');
  }
  return parsed.href.replace(/\/$/, '');
}

function requirePort(value, label) {
  if (!Number.isInteger(value) || value < 1 || value > 65_535) {
    fixtureError(`${label} must be an integer between 1 and 65535`);
  }
  return value;
}

function normalizeOptions(options) {
  const artifactRoot = options.artifactRoot
    ? requireString(options.artifactRoot, 'governance artifact root')
    : path.resolve('tmp', 'test-governance', 'ai-gateway-concurrency');
  if (!path.isAbsolute(artifactRoot)) fixtureError('governance artifact root must be absolute');
  return {
    databaseUrl: requirePostgresUrl(options.databaseUrl),
    apiServerBin: requireFile(options.apiServerBin, 'api-server binary', { executable: true }),
    pluginRunnerBin: requireFile(options.pluginRunnerBin, 'plugin-runner binary', { executable: true }),
    openaiPackage: requireFile(options.openaiPackage, 'official OpenAI package archive'),
    anthropicPackage: requireFile(options.anthropicPackage, 'official Anthropic package archive'),
    openaiCompatiblePackage: requireFile(
      options.openaiCompatiblePackage,
      'official OpenAI-compatible package archive'
    ),
    upstreamBaseUrl: requireLoopbackUrl(options.upstreamBaseUrl),
    apiPort: options.apiPort === undefined || options.apiPort === null
      ? null
      : requirePort(options.apiPort, 'api-server port'),
    readyFile: options.readyFile ? path.resolve(options.readyFile) : null,
    artifactRoot,
  };
}

module.exports = {
  GatewayFixtureError,
  normalizeOptions,
  requireLoopbackUrl,
  requirePort,
  requirePostgresUrl,
};
