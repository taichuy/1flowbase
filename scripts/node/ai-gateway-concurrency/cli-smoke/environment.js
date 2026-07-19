'use strict';

const PARENT_ALLOWLIST = Object.freeze([
  'PATH',
  'LANG',
  'LC_ALL',
  'TZ',
  'SSL_CERT_FILE',
  'SSL_CERT_DIR',
  'NODE_EXTRA_CA_CERTS',
]);

const CLEARED_CREDENTIALS = Object.freeze([
  'OPENAI_API_KEY',
  'OPENAI_API_BASE',
  'OPENAI_BASE_URL',
  'ANTHROPIC_API_KEY',
  'ANTHROPIC_AUTH_TOKEN',
  'ANTHROPIC_BASE_URL',
  'CLAUDE_CODE_OAUTH_TOKEN',
  'CLAUDE_CODE_SESSION_ACCESS_TOKEN',
  'AWS_ACCESS_KEY_ID',
  'AWS_SECRET_ACCESS_KEY',
  'AWS_SESSION_TOKEN',
  'GOOGLE_APPLICATION_CREDENTIALS',
]);

function narrowEnvironment(parentEnv, temporaryHome) {
  const env = {};
  for (const name of PARENT_ALLOWLIST) {
    if (typeof parentEnv[name] === 'string' && parentEnv[name] !== '') env[name] = parentEnv[name];
  }
  env.HOME = temporaryHome;
  env.USERPROFILE = temporaryHome;
  env.TMPDIR = temporaryHome;
  env.NO_PROXY = '127.0.0.1,localhost,::1';
  env.no_proxy = env.NO_PROXY;
  for (const name of CLEARED_CREDENTIALS) env[name] = '';
  return env;
}

function codexEnvironment(parentEnv, paths, apiKey) {
  return {
    ...narrowEnvironment(parentEnv, paths.home),
    CODEX_HOME: paths.config,
    ONEFLOWBASE_APPLICATION_API_KEY: apiKey,
  };
}

function claudeEnvironment(parentEnv, paths, gatewayBaseUrl, apiKey) {
  return {
    ...narrowEnvironment(parentEnv, paths.home),
    CLAUDE_CONFIG_DIR: paths.config,
    ANTHROPIC_BASE_URL: gatewayBaseUrl,
    ANTHROPIC_API_KEY: apiKey,
    CLAUDE_CODE_OAUTH_TOKEN: '',
  };
}

function sanitizedEnvironment(env) {
  return Object.fromEntries(Object.keys(env).sort().map((name) => [
    name,
    name.includes('KEY') || name.includes('TOKEN') || name.includes('CREDENTIAL')
      ? (env[name] ? '<ephemeral-application-key>' : '<cleared>')
      : env[name],
  ]));
}

module.exports = {
  CLEARED_CREDENTIALS,
  PARENT_ALLOWLIST,
  claudeEnvironment,
  codexEnvironment,
  narrowEnvironment,
  sanitizedEnvironment,
};
