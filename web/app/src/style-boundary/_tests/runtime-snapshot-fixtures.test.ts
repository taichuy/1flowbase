import { createHash } from 'node:crypto';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { fetchPublicLoginEntries } from '@1flowbase/api-client';
import passwordLocalSource from '../../../../../api/crates/control-plane/src/auth/public_ui_blocks/password_local.tsx?raw';
import { fetchFrontstageRuntimeI18nCatalog } from '../../features/frontstage/api/runtime-i18n';
import { createBlockI18n } from '../../features/frontstage/lib/runtime-i18n/translator';
import { fetchSettingsMembers } from '../../features/settings/api/identity-access/members';
import { fetchSettingsMcpBundleExportDefaults } from '../../features/settings/api/mcp-management';
import { seedStyleBoundaryCommonFetch } from '../scene-fixtures/common-fetch';
import { seedStyleBoundarySettingsFetch } from '../scene-fixtures/settings-fetch';
import { styleBoundaryOrganizationMembers } from '../scene-fixtures/settings/organization';

// original-fetch intentionally retains one native fallback across scene installs.
const fallback = vi.fn(async () => new Response('unhandled', { status: 503 }));
let originalFetch: typeof fetch;
let originalEventSource: typeof EventSource;

beforeEach(() => {
  originalFetch = globalThis.fetch;
  originalEventSource = globalThis.EventSource;
  fallback.mockReset();
  fallback.mockImplementation(
    async () => new Response('unhandled', { status: 503 })
  );
  globalThis.fetch = fallback;
});
afterEach(() => {
  globalThis.fetch = originalFetch;
  globalThis.EventSource = originalEventSource;
  vi.restoreAllMocks();
});

test('runtime catalog returns nonempty matching-locale raw snapshots used by real page title translation', async () => {
  seedStyleBoundaryCommonFetch();
  for (const [locale, expected] of [
    ['en_US', 'Overview'],
    ['zh_Hans', '概览']
  ] as const) {
    const response = await fetch(`/api/console/i18n/catalog?locale=${locale}`);
    const raw = await response.json();
    expect(raw).not.toHaveProperty('data');
    expect(raw.locale).toBe(locale);
    const catalog = await fetchFrontstageRuntimeI18nCatalog(locale);
    expect(catalog.catalog_revision).toBe(1);
    expect(catalog.digest).toBe(
      'sha256:' +
        createHash('sha256')
          .update(
            JSON.stringify({
              catalog_revision: catalog.catalog_revision,
              locale: catalog.locale,
              messages: catalog.messages
            })
          )
          .digest('hex')
    );
    expect(
      createBlockI18n({
        locale,
        status: 'ready',
        messages: catalog.messages
      }).t('Overview')
    ).toBe(expected);
  }
  expect(fallback).not.toHaveBeenCalled();
});

test('login discovery returns the canonical usable local password block and selects it by id', async () => {
  seedStyleBoundaryCommonFetch();
  const snapshot = await fetchPublicLoginEntries();
  expect(snapshot.login_entries).toHaveLength(1);
  const entry = snapshot.login_entries[0];
  expect(entry.id).toBe(snapshot.default_login_entry_id);
  expect(entry).toMatchObject({
    auth_type: 'password-local',
    entry_action: 'render_ui',
    is_builtin: true
  });
  expect(entry.public_ui_block).toBe(passwordLocalSource);
  expect(entry.public_ui_block).toContain(
    'export default function PasswordLocalAuth'
  );
  expect(fallback).not.toHaveBeenCalled();
});

test('settings scene members reuse the typed organization snapshot through the real consumer', async () => {
  seedStyleBoundarySettingsFetch();
  expect(await fetchSettingsMembers()).toEqual(
    styleBoundaryOrganizationMembers
  );
  expect(styleBoundaryOrganizationMembers.length).toBeGreaterThan(0);
  expect(fallback).not.toHaveBeenCalled();
});

test('MCP export defaults use the existing style runtime system version', async () => {
  seedStyleBoundarySettingsFetch();
  expect(await fetchSettingsMcpBundleExportDefaults()).toEqual({
    current_system_version: '0.2.6'
  });
  expect(fallback).not.toHaveBeenCalled();
});

test('unknown locales, paths, origins and methods retain original transport failures', async () => {
  seedStyleBoundarySettingsFetch();
  seedStyleBoundaryCommonFetch();
  const requests: Array<[string, RequestInit?]> = [
    ['/api/console/i18n/catalog?locale=unknown'],
    ['/api/console/i18n/catalog?locale=constructor'],
    ['/api/console/i18n/catalog?locale=en_US', { method: 'POST' }],
    ['https://other.example/api/console/i18n/catalog?locale=en_US'],
    ['/api/public/auth/login-entries', { method: 'POST' }],
    ['https://other.example/api/public/auth/login-entries'],
    ['/api/console/settings/members', { method: 'POST' }],
    ['https://other.example/api/console/settings/members'],
    ['/api/console/mcp/bundles/export-defaults', { method: 'POST' }],
    ['https://other.example/api/console/mcp/bundles/export-defaults'],
    ['/api/console/unhandled']
  ];
  for (const [url, init] of requests) {
    expect((await fetch(url, init)).status).toBe(503);
    expect(fallback).toHaveBeenLastCalledWith(url, init);
  }
});

test('the runtime consumer still rejects mismatched snapshots and real network failures', async () => {
  globalThis.fetch = vi.fn(
    async () =>
      new Response(
        JSON.stringify({
          catalog_revision: 1,
          locale: 'zh_Hans',
          digest: 'mismatch',
          messages: { Overview: '概览' }
        })
      )
  );
  await expect(fetchFrontstageRuntimeI18nCatalog('en_US')).rejects.toThrow(
    'different locale'
  );
  const networkError = new Error('runtime API unavailable');
  globalThis.fetch = vi.fn(async () => {
    throw networkError;
  });
  await expect(fetchFrontstageRuntimeI18nCatalog('en_US')).rejects.toBe(
    networkError
  );
});
