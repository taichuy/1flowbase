import { describe, expect, test, vi } from 'vitest';

import {
  getConsoleApplicationCatalog,
  listConsoleApplicationManagement,
  type ConsoleApplicationCatalog
} from '../console/applications';
import {
  getConsoleApplicationNodeCatalog,
  type ConsoleApplicationNodeCatalog
} from '../console-node-contributions';
import * as transport from '../transport';

describe('console application management client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('AC-006 serializes resource filters, sorting, and pagination', async () => {
    await expect(
      listConsoleApplicationManagement({
        page: 2,
        page_size: 20,
        filter: {
          application_type: 'workflow',
          publication_status: 'unpublished'
        },
        sort: 'updated_at:desc'
      })
    ).resolves.toMatchObject({
      path: '/api/console/settings/applications?page=2&page_size=20&filter=%7B%22application_type%22%3A%22workflow%22%2C%22publication_status%22%3A%22unpublished%22%7D&sort=updated_at%3Adesc'
    });
  });

  test('AC-004 requests the Application type and Workflow trigger catalog', async () => {
    const fixture = {
      types: [
        {
          value: 'workflow',
          label: 'Workflow'
        }
      ],
      workflow_triggers: [
        {
          value: 'extension',
          label: 'Extension'
        }
      ],
      tags: [],
      collectors: [
        {
          collector_code: 'codex-logs-collector',
          source_client: 'codex',
          display_name: 'Codex',
          description: 'Collect local sessions',
          version: '0.2.0',
          execution_target: 'client',
          catalog_id: 'runtime-extensions:taichuy/codex-logs-collector',
          category: 'runtime-extensions',
          installation_status: 'missing',
          installed_version: '0.1.0',
          extension_installation_id: 'retained',
          installable: true,
          can_install: false,
          can_update: false,
          asset_base_url: null,
          documentation_url: null,
          shell_installer_url: null,
          powershell_installer_url: null
        }
      ]
    } satisfies ConsoleApplicationCatalog;

    expect(fixture.workflow_triggers[0]).toEqual({
      value: 'extension',
      label: 'Extension'
    });
    await expect(getConsoleApplicationCatalog()).resolves.toMatchObject({
      path: '/api/console/applications/catalog'
    });
    vi.mocked(transport.apiFetch).mockResolvedValueOnce(fixture);
    const catalog = await getConsoleApplicationCatalog();
    expect(catalog).toEqual(fixture);
    // A newer catalog package does not repair missing local assets or grant
    // installation permission; retain the old installation and null URLs.
    expect(catalog.collectors[0]).toMatchObject({
      installation_status: 'missing',
      version: '0.2.0',
      installed_version: '0.1.0',
      extension_installation_id: 'retained',
      installable: true,
      can_install: false,
      can_update: false,
      asset_base_url: null,
      documentation_url: null,
      shell_installer_url: null,
      powershell_installer_url: null
    });
    expect(catalog.collectors[0]).not.toHaveProperty('installed');
  });

  test('AC-004 requests the unified Application node catalog with exact contract fields', async () => {
    const fixture = {
      nodes: [
        {
          source_kind: 'builtin',
          node_type: 'workflow_start',
          title: 'Workflow Start',
          category: 'io',
          authoring_status: 'published',
          runtime_status: 'ready',
          dependency_status: 'not_applicable',
          field_contract: {
            config_fields: [
              {
                key: 'config.input_fields[].inputType',
                required: true,
                value_types: ['string'],
                allowed_values: ['text', 'paragraph', 'select']
              }
            ],
            input_fields: [],
            output_fields: []
          },
          plugin: null
        }
      ]
    } satisfies ConsoleApplicationNodeCatalog;

    expect(fixture.nodes[0].field_contract.config_fields[0]).toMatchObject({
      key: 'config.input_fields[].inputType',
      required: true,
      allowed_values: ['text', 'paragraph', 'select']
    });
    await expect(
      getConsoleApplicationNodeCatalog('application-1')
    ).resolves.toMatchObject({
      path: '/api/console/node-contributions?application_id=application-1'
    });
  });
});
