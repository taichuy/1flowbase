const test = require('node:test');
const assert = require('node:assert/strict');

const {
  buildAdvisoryMessage,
  main,
  routeChangedFiles,
} = require('../core.js');

test('routeChangedFiles recommends related gates for frontend and backend consistency changes', () => {
  const routes = routeChangedFiles([
    'web/app/src/pages/workspaces/WorkspacePage.tsx',
    'api/crates/orchestration-runtime/src/execution_engine.rs',
  ]);

  assert.deepEqual(routes.map((route) => route.scope), [
    'repo-frontend-pr',
    'repo-backend-static',
    'repo-backend-fmt',
    'repo-backend-check-runtime-storage',
    'backend-consistency',
  ]);
});

test('buildAdvisoryMessage produces an English ASCII non-blocking prompt', () => {
  const routes = routeChangedFiles([
    'web/app/src/i18n/en-US.json',
    'api/crates/storage-postgres/src/repository.rs',
  ]);
  const message = buildAdvisoryMessage({
    mode: 'staged',
    changedFiles: [
      'web/app/src/i18n/en-US.json',
      'api/crates/storage-postgres/src/repository.rs',
    ],
    routes,
  });

  assert.match(message, /\[1flowbase-gate-router\] Advisory only/u);
  assert.match(message, /This hook does not block the commit/u);
  assert.match(message, /repo-tooling/u);
  assert.match(message, /repo-frontend-pr/u);
  assert.match(message, /backend-consistency/u);
  assert.doesNotMatch(message, /[^\x00-\x7F]/u);
});

test('routeChangedFiles does not route frontend state names to backend consistency', () => {
  const routes = routeChangedFiles([
    'web/app/src/features/workspace/useWorkspaceState.ts',
  ]);

  assert.deepEqual(routes.map((route) => route.scope), [
    'repo-frontend-pr',
  ]);
});

test('routeChangedFiles recommends state protocol gates for Anthropic ACP projection changes', () => {
  const routes = routeChangedFiles([
    'api/apps/api-server/src/routes/application_public_api/compat_sse/protocol_mappers/anthropic_stream.rs',
    'scripts/node/acp-claude-smoke/core.js',
  ]);

  assert(routes.some((route) => route.scope === 'state-protocols'));
  assert(
    routes.some((route) =>
      route.command === 'node scripts/node/verify-state-protocols.js'
    )
  );
  assert(routes.some((route) => route.scope === 'foundation-contract-ai-gateway-fast'));
});

test('AC-001 gate router exposes foundation-specific evidence instead of generic green', () => {
  const routes = routeChangedFiles([
    'api/apps/api-server/src/routes/mcp_protocol.rs',
    'web/packages/page-runtime/src/native-react-compiler/source-contract.ts',
  ]);

  assert(routes.some((route) => route.scope === 'foundation-contract-mcp-gateway-fast'));
  assert(routes.some((route) => route.scope === 'foundation-contract-native-react-fast'));
});

test('main reads staged changes from env override and never blocks the commit', async () => {
  let stdout = '';

  const status = await main(['--staged'], {
    env: {
      GATE_ROUTER_CHANGED_FILES: [
        'web/app/src/components/Toolbar.tsx',
        'api/apps/api-server/src/routes/applications.rs',
      ].join('\n'),
    },
    writeStdout(text) {
      stdout += text;
    },
    writeStderr() {},
  });

  assert.equal(status, 0);
  assert.match(stdout, /repo-frontend-pr/u);
  assert.match(stdout, /repo-backend-check-apps/u);
  assert.match(stdout, /This hook does not block the commit/u);
});
