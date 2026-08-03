import { beforeEach, describe, expect, test, vi } from 'vitest';

import * as transport from '../../transport';
import {
  deleteConsoleInstalledExtension,
  checkConsoleExtensionUpdates,
  getConsoleExtensionCatalogEntry,
  getConsoleExtensionRiskChallenge,
  installConsoleExtension,
  listConsoleExtensionCatalog,
  listConsoleInstalledExtensions,
  selectConsoleInstalledExtension,
  uploadConsoleExtension,
  type ConsoleInstalledExtensionPage
} from '../extensions';
import { ApiClientError } from '../../errors';

describe('extension center client contract', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(transport, 'apiFetch').mockImplementation(
      async (input) => input as never
    );
  });

  test('D4-AC-001 keeps installed inventory local and paginated', async () => {
    await expect(
      listConsoleInstalledExtensions('cursor-1', 20, 'agent-flow')
    ).resolves.toMatchObject({
      path: '/api/console/settings/extension-center/installed?limit=20&cursor=cursor-1&category=agent-flow'
    });
  });

  test('D4-AC-013 keeps family pagination and version history in the backend DTO', () => {
    const page = {
      limit: 20,
      total_entries: 1,
      next_cursor: null,
      entries: [
        {
          id: 'installation-current',
          catalog_id: 'runtime-extensions:taichuy/anthropic',
          category: 'runtime-extensions',
          organization: 'taichuy',
          artifact_id: 'anthropic',
          version: '0.1.23',
          node_id: 'node-a',
          source_kind: 'upload',
          trust_level: 'unknown',
          warnings: [],
          local_path: '/api/plugins/anthropic/0.1.23',
          expected_checksum: 'sha256:current',
          local_checksum: 'sha256:current',
          signature_status: 'missing',
          signature_algorithm: null,
          signing_key_id: null,
          status: 'installed',
          is_current: true,
          application_action: 'none',
          application_status: 'not_required',
          created_by: 'user-1',
          created_at: '2026-08-01T00:00:00Z',
          updated_at: '2026-08-01T00:00:00Z',
          installed_versions: [
            {
              id: 'installation-current',
              version: '0.1.23',
              source_kind: 'upload',
              trust_level: 'unknown',
              warnings: [],
              local_path: '/api/plugins/anthropic/0.1.23',
              expected_checksum: 'sha256:current',
              local_checksum: 'sha256:current',
              signature_status: 'missing',
              signature_algorithm: null,
              signing_key_id: null,
              status: 'installed',
              is_current: true,
              deletable: false,
              delete_reasons: ['current_version'],
              created_by: 'user-1',
              created_at: '2026-08-01T00:00:00Z',
              updated_at: '2026-08-01T00:00:00Z'
            }
          ]
        }
      ]
    } satisfies ConsoleInstalledExtensionPage;

    expect(page.total_entries).toBe(1);
    expect(page.entries[0].installed_versions).toHaveLength(1);
  });

  test('AC-005 selects and deletes an exact installed version', async () => {
    await expect(
      selectConsoleInstalledExtension('installation old', 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/extension-center/installed/installation%20old/select',
      method: 'POST',
      csrfToken: 'csrf'
    });
    await expect(
      deleteConsoleInstalledExtension('installation old', 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/extension-center/installed/installation%20old',
      method: 'DELETE',
      csrfToken: 'csrf'
    });
  });

  test('D4-AC-002 addresses repository category catalog pages directly', async () => {
    await expect(
      listConsoleExtensionCatalog('runtime-extensions', 'page-2', 20)
    ).resolves.toMatchObject({
      path: '/api/console/settings/extension-center/catalog/runtime-extensions?limit=20&cursor=page-2'
    });
  });

  test('D4-AC-003 checks only the supplied current category page', async () => {
    await expect(
      checkConsoleExtensionUpdates(
        {
          category: 'runtime-extensions',
          catalog_page: 'page-2',
          items: [
            {
              catalog_id: 'runtime-extensions:taichuy/openai',
              current_version: '1.0.0',
              installed_versions: ['1.0.0']
            }
          ]
        },
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/extension-center/update-check',
      method: 'POST',
      csrfToken: 'csrf'
    });
  });

  test('D4-AC-004 submits explicit warning overrides without metadata inference', async () => {
    await expect(
      installConsoleExtension(
        {
          category: 'runtime-extensions',
          catalog_id: 'runtime-extensions:taichuy/openai',
          version: '1.1.0',
          compatibility_override: {
            reason: 'below_minimum_host_version',
            acknowledged_current_host_version: '0.3.1',
            acknowledged_minimum_host_version: '0.4.0'
          },
          risk_override: {
            reason: 'user_confirmed',
            acknowledged_warnings: ['signature_invalid']
          }
        },
        'csrf',
        true
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/extension-center/update',
      method: 'POST',
      body: {
        category: 'runtime-extensions',
        catalog_id: 'runtime-extensions:taichuy/openai',
        version: '1.1.0'
      },
      csrfToken: 'csrf'
    });
  });

  test('Root-AC-004 resolves an installed row through the exact catalog detail endpoint', async () => {
    await expect(
      getConsoleExtensionCatalogEntry(
        'runtime-extensions',
        'runtime-extensions:taichuy/model provider'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/extension-center/catalog/runtime-extensions/runtime-extensions%3Ataichuy%2Fmodel%20provider'
    });
  });

  test('Root-AC-006 uploads a package and retries with exact challenge overrides', async () => {
    const file = new File(['extension'], 'extension.1flowbasepkg');
    await expect(
      uploadConsoleExtension(
        file,
        {
          category: 'agent-flow',
          organization: '@taichuy',
          artifact_id: 'sample-flow',
          version: '1.2.0'
        },
        'csrf',
        {
          risk_override: {
            reason: 'user_confirmed',
            acknowledged_warnings: ['signature_invalid']
          }
        }
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/extension-center/install-upload',
      method: 'POST',
      csrfToken: 'csrf'
    });

    const request = vi.mocked(transport.apiFetch).mock.calls.at(-1)?.[0];
    expect(request?.rawBody).toBeInstanceOf(FormData);
    expect((request?.rawBody as FormData).get('file')).toBe(file);
    expect((request?.rawBody as FormData).get('category')).toBe('agent-flow');
    expect((request?.rawBody as FormData).get('organization')).toBe('@taichuy');
    expect((request?.rawBody as FormData).get('artifact_id')).toBe(
      'sample-flow'
    );
    expect((request?.rawBody as FormData).get('version')).toBe('1.2.0');
    expect((request?.rawBody as FormData).get('risk_override')).toBe(
      JSON.stringify({
        reason: 'user_confirmed',
        acknowledged_warnings: ['signature_invalid']
      })
    );
  });

  test('Root-AC-006 exposes the backend risk challenge without deriving warnings in the client', () => {
    const challenge = {
      warnings: [
        {
          code: 'signature_invalid',
          message: 'The package signature does not match its contents.',
          overridable: true
        }
      ],
      compatibility: null
    };
    const error = new ApiClientError({
      status: 409,
      code: 'extension_risk_confirmation_required',
      message: 'confirmation required',
      body: {
        status: 409,
        code: 'extension_risk_confirmation_required',
        message: 'confirmation required',
        risk_challenge: challenge
      }
    });

    expect(getConsoleExtensionRiskChallenge(error)).toEqual(challenge);
  });
});
