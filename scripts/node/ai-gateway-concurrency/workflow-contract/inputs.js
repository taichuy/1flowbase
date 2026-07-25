'use strict';

const fs = require('node:fs');
const path = require('node:path');

const FULL_SHA_PATTERN = /^[0-9a-f]{40}$/u;

function requireFullSha(value, label) {
  if (typeof value !== 'string' || !FULL_SHA_PATTERN.test(value)) {
    throw new Error(`${label} must be a full lowercase 40-character hex SHA`);
  }
  return value;
}

function requireCharacterizeProfile(value) {
  if (value !== 'characterize') {
    throw new Error('profile must be characterize; regression has no approved checked-in budget');
  }
  return value;
}

function requireFile(value, label, executable = false) {
  if (typeof value !== 'string' || value.trim() === '') throw new Error(`${label} is required`);
  const resolved = path.resolve(value);
  let stat;
  try {
    stat = fs.statSync(resolved);
  } catch {
    throw new Error(`${label} does not exist`);
  }
  if (!stat.isFile()) throw new Error(`${label} must be a file`);
  if (executable) fs.accessSync(resolved, fs.constants.X_OK);
  return resolved;
}

function requireDirectory(value, label) {
  if (typeof value !== 'string' || value.trim() === '') throw new Error(`${label} is required`);
  const resolved = path.resolve(value);
  if (!fs.statSync(resolved).isDirectory()) throw new Error(`${label} must be a directory`);
  return resolved;
}

function singlePackage(directory, provider) {
  const packages = [];
  const visit = (current) => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const candidate = path.join(current, entry.name);
      if (entry.isDirectory()) visit(candidate);
      else if (entry.isFile() && entry.name.endsWith('.1flowbasepkg')) packages.push(candidate);
    }
  };
  visit(requireDirectory(directory, `official ${provider} package directory`));
  if (packages.length !== 1) throw new Error(`official ${provider} package directory must contain exactly one package`);
  return packages[0];
}

function requirePostgresUrl(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error('database URL must be a valid PostgreSQL URL');
  }
  if (!['postgres:', 'postgresql:'].includes(url.protocol) || !url.hostname || !url.pathname.slice(1)) {
    throw new Error('database URL must name a PostgreSQL host and database');
  }
  return value;
}

function requireText(value, label) {
  if (typeof value !== 'string' || !value.trim()) throw new Error(`${label} is required`);
  return value.trim();
}

function normalizeRunInputs(options) {
  return {
    mainSourceSha: requireFullSha(options.mainSourceSha, 'main source SHA'),
    officialSourceSha: requireFullSha(options.officialSourceSha, 'official source SHA'),
    profile: requireCharacterizeProfile(options.profile),
    repoRoot: requireDirectory(options.repoRoot, 'repository root'),
    databaseUrl: requirePostgresUrl(options.databaseUrl),
    apiServerBin: requireFile(options.apiServerBin, 'api-server binary', true),
    pluginRunnerBin: requireFile(options.pluginRunnerBin, 'plugin-runner binary', true),
    openaiPackage: singlePackage(options.openaiPackageDir, 'OpenAI'),
    anthropicPackage: singlePackage(options.anthropicPackageDir, 'Anthropic'),
    codexExecutable: requireFile(options.codexExecutable, 'Codex executable', true),
    claudeExecutable: requireFile(options.claudeExecutable, 'Claude executable', true),
    hostTarget: requireText(options.hostTarget, 'host target'),
  };
}

module.exports = {
  FULL_SHA_PATTERN,
  normalizeRunInputs,
  requireCharacterizeProfile,
  requireFullSha,
  singlePackage,
};
