'use strict';

const fs = require('node:fs');
const path = require('node:path');

const SCHEMA = '1flowbase.local-count-tokens-upgrade/v1';
const EVIDENCE_SCHEMA = '1flowbase.local-count-tokens-upgrade-evidence/v1';
const REQUIRED_STEPS = Object.freeze([
  'count_tokens_through_published_application',
  'claude_conversation_turn',
  'upgrade_plugin_without_republish',
  'claude_followup_turn',
]);
const FORBIDDEN_PORTS = Object.freeze([3100, 7800, 7801]);

function requireCondition(value, message) {
  if (!value) throw new Error(message);
}

function loadCountTokensUpgradeFixture(filePath = path.join(__dirname, 'count-tokens-upgrade.fixture.json')) {
  const fixture = JSON.parse(fs.readFileSync(filePath, 'utf8'));
  requireCondition(fixture.schema_version === SCHEMA, 'local CountTokens upgrade fixture schema mismatch');
  requireCondition(fixture.provider === 'deepseek' && fixture.client === 'claude',
    'local CountTokens upgrade fixture must bind Claude Code to DeepSeek');
  requireCondition(JSON.stringify(fixture.steps) === JSON.stringify(REQUIRED_STEPS),
    'local CountTokens upgrade fixture step order mismatch');
  requireCondition(fixture.published_application?.reuse_same_application === true,
    'local CountTokens upgrade fixture must reuse one published application');
  requireCondition(fixture.published_application?.reuse_same_publication === true,
    'local CountTokens upgrade fixture must not republish');
  requireCondition(fixture.published_application?.count_tokens_path === '/v1/messages/count_tokens'
    && fixture.published_application?.conversation_path === '/v1/messages',
  'local CountTokens upgrade fixture public paths mismatch');
  requireCondition(fixture.upgrade?.republish === false && fixture.upgrade?.network_install === false,
    'local CountTokens upgrade fixture must use a local upgrade without republish');
  requireCondition(fixture.resources?.reuse_local_acceptance_provenance === true
    && fixture.resources?.reuse_owned_tmux === true
    && fixture.resources?.finally_cleanup === true,
  'local CountTokens upgrade fixture must reuse provenance, owned tmux, and finally cleanup');
  requireCondition(JSON.stringify(fixture.resources?.forbidden_ports) === JSON.stringify(FORBIDDEN_PORTS),
    'local CountTokens upgrade fixture protected ports mismatch');
  requireCondition(fixture.artifact.startsWith('tmp/test-governance/'),
    'local CountTokens upgrade evidence must be under tmp/test-governance');
  return fixture;
}

function countTokensResult(value) {
  requireCondition(Number.isSafeInteger(value.input_tokens) && value.input_tokens >= 0,
    'CountTokens evidence omitted an unsigned input_tokens total');
  return { input_tokens: value.input_tokens };
}

function buildCountTokensUpgradeEvidence(fixture, observed) {
  requireCondition(observed.application_id && observed.application_id === observed.after_upgrade_application_id,
    'plugin upgrade changed the published application');
  requireCondition(observed.publication_id && observed.publication_id === observed.after_upgrade_publication_id,
    'plugin upgrade republished the application');
  requireCondition(observed.before_plugin?.package_sha256 && observed.after_plugin?.package_sha256,
    'plugin upgrade evidence omitted package provenance');
  requireCondition(observed.before_plugin.package_sha256 !== observed.after_plugin.package_sha256,
    'plugin upgrade evidence did not change the installed package');
  requireCondition(observed.republish_events === 0, 'plugin upgrade emitted a republish event');
  requireCondition(observed.network_installs === 0, 'plugin upgrade used a network install');
  requireCondition(observed.count_tokens_application_id === observed.application_id,
    'CountTokens did not use the published DeepSeek application');
  requireCondition(observed.claude?.application_id === observed.application_id,
    'Claude Code did not continue through the same published DeepSeek application');
  requireCondition(observed.claude?.surface === 'tmux' && observed.claude?.turns === 2,
    'Claude Code evidence must contain the initial and follow-up turns on owned tmux');
  requireCondition(observed.claude?.continued_session === true,
    'Claude Code follow-up did not continue the same conversation');
  requireCondition(observed.cleanup?.status === 'pass', 'local CountTokens upgrade cleanup failed');
  return {
    schema_version: EVIDENCE_SCHEMA,
    status: 'pass',
    provider: fixture.provider,
    client: fixture.client,
    application_id: observed.application_id,
    publication_id: observed.publication_id,
    count_tokens: countTokensResult(observed.count_tokens),
    claude: observed.claude,
    plugin_upgrade: {
      before: observed.before_plugin,
      after: observed.after_plugin,
      republish_events: observed.republish_events,
      network_installs: observed.network_installs,
    },
    runtime: observed.runtime,
    cleanup: observed.cleanup,
  };
}

module.exports = {
  EVIDENCE_SCHEMA,
  FORBIDDEN_PORTS,
  REQUIRED_STEPS,
  buildCountTokensUpgradeEvidence,
  loadCountTokensUpgradeFixture,
};
