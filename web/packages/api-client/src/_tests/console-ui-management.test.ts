import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';
import {
  fetchConsoleUiTemplates,
  publishConsoleUiTemplate,
  resetConsoleUiTemplateDefault,
  updateConsoleUiComponentState
} from '../console-ui-management';

describe('console UI management client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('uses the distinct template management URL', async () => {
    await expect(fetchConsoleUiTemplates(true)).resolves.toMatchObject({
      path: '/api/console/settings/ui-management/templates?include_archived=true'
    });
  });

  test('publishes an immutable template revision', async () => {
    await expect(
      publishConsoleUiTemplate('template-1', 3, 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/ui-management/templates/template-1/publish',
      method: 'POST',
      body: { revision: 3 },
      csrfToken: 'csrf'
    });
  });

  test('restores the official default by stable contribution locator', async () => {
    await expect(
      resetConsoleUiTemplateDefault(
        {
          provider_code: '1flowbase',
          contribution_code: 'frontstage.js-ui-block'
        },
        'csrf'
      )
    ).resolves.toMatchObject({
      method: 'DELETE',
      body: {
        provider_code: '1flowbase',
        contribution_code: 'frontstage.js-ui-block'
      }
    });
  });

  test('changes discovery state without addressing runtime assets', async () => {
    await expect(
      updateConsoleUiComponentState(
        {
          provider_code: '1flowbase',
          contribution_code: 'frontstage.js-ui-block',
          module_source: 'antd',
          export_name: 'Button'
        },
        'hidden',
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/ui-management/components/state',
      method: 'PUT',
      body: expect.objectContaining({ state: 'hidden' })
    });
  });
});
