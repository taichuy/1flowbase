import { describe, expect, test, vi } from 'vitest';

import {
  getConsoleApplicationCatalog,
  listConsoleApplicationManagement,
  type ConsoleApplicationCatalog
} from '../console/applications';
import {
  listConsoleNodeContributions,
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
          label: 'Workflow',
          description: 'Runs a typed Workflow graph.'
        }
      ],
      workflow_triggers: [
        {
          value: 'extension',
          label: 'Extension',
          description: 'Invoked through a published extension route.'
        }
      ],
      tags: []
    } satisfies ConsoleApplicationCatalog;

    expect(fixture.workflow_triggers[0].description).toContain('extension');
    await expect(getConsoleApplicationCatalog()).resolves.toMatchObject({
      path: '/api/console/applications/catalog'
    });
  });

  test('AC-004 requests the unified Application node catalog with exact contract fields', async () => {
    const fixture = {
      nodes: [
        {
          source_kind: 'builtin',
          node_type: 'workflow_start',
          title: 'Workflow Start',
          description: 'Defines Workflow input fields.',
          category: 'io',
          runtime_status: 'ready',
          runtime_status_description:
            'Executable by the current orchestration runtime.',
          dependency_status: 'not_applicable',
          field_contract: {
            config_fields: [
              {
                key: 'config.input_fields[].inputType',
                description: 'Authoring control type.',
                required: true,
                value_types: ['string'],
                allowed_values: ['text', 'paragraph', 'select'],
                applicability: null
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
      listConsoleNodeContributions('application-1')
    ).resolves.toMatchObject({
      path: '/api/console/node-contributions?application_id=application-1'
    });
  });
});
