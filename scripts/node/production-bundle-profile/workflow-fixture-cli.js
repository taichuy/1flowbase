#!/usr/bin/env node

const path = require('node:path');

const {
  loadRootCredentials,
  openTemporaryOwnerSession,
} = require('../page-debug/auth.js');

const FIXTURE_PREFIX = 'qa-bundle-graph-';

function optionValue(name, fallback) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

async function apiRequest(apiBaseUrl, session, requestPath, init = {}) {
  const response = await fetch(`${apiBaseUrl}${requestPath}`, {
    ...init,
    headers: {
      cookie: session.cookie,
      ...(init.method && init.method !== 'GET'
        ? { 'x-csrf-token': session.csrfToken }
        : {}),
      ...(init.body ? { 'content-type': 'application/json' } : {}),
      ...init.headers,
    },
  });
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`${init.method || 'GET'} ${requestPath}: ${response.status} ${text.slice(0, 500)}`);
  }
  return text ? JSON.parse(text) : null;
}

async function main() {
  const action = process.argv[2];
  if (!['create', 'delete'].includes(action)) {
    throw new Error('Expected action: create or delete');
  }
  const repoRoot = path.resolve(__dirname, '../../..');
  const apiBaseUrl = optionValue('--api-base-url', 'http://127.0.0.1:3300');
  const credentials = loadRootCredentials({ repoRoot });
  const session = await openTemporaryOwnerSession({
    apiBaseUrl,
    account: credentials.account,
    password: credentials.password,
  });
  try {
    if (action === 'create') {
      const name = `${FIXTURE_PREFIX}${Date.now()}`;
      const response = await apiRequest(
        apiBaseUrl,
        session,
        '/api/console/applications',
        {
          method: 'POST',
          body: JSON.stringify({
            application_type: 'agent_flow',
            name,
            description: 'Ephemeral production bundle graph fixture',
            icon: null,
            icon_type: null,
            icon_background: null,
          }),
        },
      );
      const application = response?.data ?? response;
      if (!application?.id || application.name !== name) {
        throw new Error('Application fixture response has no matching identity');
      }
      process.stdout.write(`${JSON.stringify({ id: application.id, name })}\n`);
      return;
    }

    const id = optionValue('--id');
    if (!id) throw new Error('delete requires --id');
    const response = await apiRequest(
      apiBaseUrl,
      session,
      `/api/console/applications/${id}`,
    );
    const application = response?.data ?? response;
    if (!String(application?.name || '').startsWith(FIXTURE_PREFIX)) {
      throw new Error('Refusing to delete an application not owned by this fixture');
    }
    await apiRequest(
      apiBaseUrl,
      session,
      `/api/console/applications/${id}`,
      { method: 'DELETE' },
    );
    process.stdout.write(`${JSON.stringify({ deleted: id })}\n`);
  } finally {
    await session.dispose();
  }
}

main().catch((error) => {
  process.stderr.write(`${error.stack || error}\n`);
  process.exitCode = 1;
});
