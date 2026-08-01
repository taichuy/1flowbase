import { beforeEach, describe, expect, test, vi } from 'vitest';

import * as transport from '../../transport';
import {
  checkConsoleExtensionUpdates,
  installConsoleExtension,
  listConsoleExtensionCatalog,
  listConsoleInstalledExtensions
} from '../extensions';

describe('extension center client contract', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(transport, 'apiFetch').mockImplementation(async (input) => input as never);
  });

  test('D4-AC-001 keeps installed inventory local and paginated', async () => {
    await expect(listConsoleInstalledExtensions('cursor-1', 20)).resolves.toMatchObject({
      path: '/api/console/settings/extension-center/installed?limit=20&cursor=cursor-1'
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
          items: [{ artifact_id: '1flowbase.openai', current_version: '1.0.0' }]
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
          artifact_id: '1flowbase.openai',
          artifact_kind: 'model_provider',
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
      csrfToken: 'csrf'
    });
  });
});
