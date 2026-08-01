'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  buildCountTokensUpgradeEvidence,
  loadCountTokensUpgradeFixture,
} = require('../count-tokens-upgrade');

function observed() {
  return {
    application_id: 'published-deepseek-app',
    after_upgrade_application_id: 'published-deepseek-app',
    publication_id: 'published-deepseek-v1',
    after_upgrade_publication_id: 'published-deepseek-v1',
    before_plugin: { plugin_id: 'deepseek@0.1.17', package_sha256: 'sha256:before' },
    after_plugin: { plugin_id: 'deepseek@0.1.18', package_sha256: 'sha256:after' },
    republish_events: 0,
    network_installs: 0,
    count_tokens_application_id: 'published-deepseek-app',
    count_tokens: {
      operation: 'count_tokens', input_tokens: 41, method: 'provider_estimate',
      coverage: 'complete', unknown_block_count: 0,
    },
    claude: {
      application_id: 'published-deepseek-app', surface: 'tmux', turns: 2,
      continued_session: true,
    },
    cleanup: { status: 'pass', owned_tmux_servers: 0, owned_processes: 0 },
  };
}

test('Root #1556 P13 freezes CountTokens, conversation, local upgrade, and no-republish evidence', () => {
  const fixture = loadCountTokensUpgradeFixture();
  const evidence = buildCountTokensUpgradeEvidence(fixture, observed());
  assert.equal(evidence.status, 'pass');
  assert.equal(evidence.count_tokens.input_tokens, 41);
  assert.equal(evidence.plugin_upgrade.republish_events, 0);
});

test('Root #1556 P13 controlled negative rejects a publication change during plugin upgrade', () => {
  const value = observed();
  value.after_upgrade_publication_id = 'republished-deepseek-v2';
  assert.throws(
    () => buildCountTokensUpgradeEvidence(loadCountTokensUpgradeFixture(), value),
    /republished the application/u,
  );
});
