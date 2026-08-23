import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';
import {
  createConsoleUiComponent,
  downloadConsoleUiCatalogComponent,
  deleteConsoleUiComponent,
  fetchConsoleUiComponent,
  fetchConsoleUiComponents,
  fetchConsoleUiCatalogPage,
  fetchConsoleUiCatalogIndex,
  fetchConsoleUiCatalogUpdateStatus,
  searchConsoleUiCatalog,
  fetchConsoleUiTemplates,
  publishConsoleUiTemplate,
  resetConsoleUiTemplateDefault,
  syncConsoleUiCatalogGroup,
  updateConsoleUiComponent
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

  test('uses record CRUD paths without legacy contract or state endpoints', async () => {
    await expect(fetchConsoleUiComponents()).resolves.toMatchObject({
      path: '/api/console/settings/ui-management/components'
    });
    await expect(fetchConsoleUiComponent('component-1')).resolves.toMatchObject(
      {
        path: '/api/console/settings/ui-management/components/component-1'
      }
    );
    await expect(
      createConsoleUiComponent(
        {
          component_code: 'local.button',
          name: 'Button',
          description: 'Button example',
          import_code: "import { Button } from 'antd';",
          source_code: '<Button />',
          source: 'local',
          group: 'controls',
          upstream: { identity: 'antd', version: '6.0.0' },
          version: '1.0.0',
          keywords: ['action']
        },
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/ui-management/components',
      method: 'POST'
    });
    await expect(
      updateConsoleUiComponent(
        'component-1',
        {
          name: 'Primary button',
          description: 'Button example',
          import_code: "import { Button } from 'antd';",
          source_code: '<Button type="primary" />',
          source: 'local',
          group: 'controls',
          upstream: { identity: 'antd', version: '6.0.0' },
          version: '1.1.0',
          keywords: ['action']
        },
        'csrf'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/ui-management/components/component-1',
      method: 'PUT'
    });
    await expect(
      deleteConsoleUiComponent('component-1', 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/ui-management/components/component-1',
      method: 'DELETE'
    });
  });

  test('uses remote catalog browse, search, update, download and group sync contracts', async () => {
    await expect(fetchConsoleUiCatalogIndex()).resolves.toMatchObject({
      path: '/api/console/settings/ui-management/components/catalog/index'
    });
    await expect(fetchConsoleUiCatalogPage(2)).resolves.toMatchObject({
      path: '/api/console/settings/ui-management/components/catalog/pages/2'
    });
    await expect(
      searchConsoleUiCatalog('chat input', 3, 20)
    ).resolves.toMatchObject({
      path: '/api/console/settings/ui-management/components/catalog/search?q=chat%20input&page=3&page_size=20'
    });
    await expect(fetchConsoleUiCatalogUpdateStatus()).resolves.toMatchObject({
      path: '/api/console/settings/ui-management/components/catalog/update-status'
    });
    await expect(
      downloadConsoleUiCatalogComponent('taichuy.ant-design-x.sender', 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/ui-management/components/catalog/taichuy.ant-design-x.sender/download',
      method: 'POST',
      csrfToken: 'csrf'
    });
    await expect(
      syncConsoleUiCatalogGroup('taichuy', 'ant-design-x', 'csrf')
    ).resolves.toMatchObject({
      path: '/api/console/settings/ui-management/components/catalog/groups/taichuy/ant-design-x/sync',
      method: 'POST',
      csrfToken: 'csrf'
    });
  });
});
