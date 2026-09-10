import { describe, expect, test, vi } from 'vitest';
import * as apiClient from '../index';
import * as transport from '../transport';

import {
  exportConsoleApplicationArchive,
  importConsoleApplicationArchive,
  importConsoleInstalledApplicationExtension,
  previewConsoleApplicationArchive,
  previewConsoleInstalledApplicationExtension
} from '../console/application-orchestration';

describe('console application orchestration client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );
  vi.spyOn(transport, 'apiFetchBlob').mockImplementation(
    async (input) => input as never
  );

  test('exports selected applications through the cardinality-aware archive route', async () => {
    await expect(
      exportConsoleApplicationArchive(
        { application_ids: ['app-1', 'app-2'] },
        'https://api.flowbase.test'
      )
    ).resolves.toMatchObject({
      path: '/api/console/applications/archive/export',
      method: 'POST',
      body: { application_ids: ['app-1', 'app-2'] },
      baseUrl: 'https://api.flowbase.test'
    });
  });

  test('previews and imports application archives as multipart files', async () => {
    const file = new Blob(['archive'], { type: 'application/zip' });

    await expect(
      previewConsoleApplicationArchive(
        file,
        'application.zip',
        'https://api.flowbase.test'
      )
    ).resolves.toMatchObject({
      path: '/api/console/applications/archive/preview',
      method: 'POST',
      rawBody: expect.any(FormData),
      baseUrl: 'https://api.flowbase.test'
    });
    await expect(
      importConsoleApplicationArchive(
        {
          file,
          filename: 'application.zip',
          name: 'Imported application'
        },
        'csrf-123',
        'https://api.flowbase.test'
      )
    ).resolves.toMatchObject({
      path: '/api/console/applications/archive/import',
      method: 'POST',
      rawBody: expect.any(FormData),
      csrfToken: 'csrf-123',
      baseUrl: 'https://api.flowbase.test'
    });
  });

  test('sends batch entry indices and names in one multipart import', async () => {
    const applications = [
      { entry_index: 0, name: 'Workflow A' },
      { entry_index: 3, name: 'Workflow D' }
    ];
    await importConsoleApplicationArchive(
      {
        file: new Blob(['zip']),
        filename: 'applications-4-items.zip',
        applications
      },
      'csrf'
    );
    const options = vi.mocked(transport.apiFetch).mock.calls.at(-1)![0];
    expect((options.rawBody as FormData).get('applications')).toBe(
      JSON.stringify(applications)
    );
    expect(options.csrfToken).toBe('csrf');
  });

  test('does not export removed official Agent Flow route clients', () => {
    for (const exportName of [
      'listConsoleOfficialAgentFlowTemplateCatalog',
      'syncConsoleOfficialAgentFlowTemplate',
      'previewConsoleOfficialAgentFlowTemplate',
      'importConsoleOfficialAgentFlowTemplate',
      'switchConsoleOfficialAgentFlowTemplateCurrent',
      'deleteConsoleOfficialAgentFlowTemplateRelease',
      'repairConsoleOfficialAgentFlowTemplateRelease'
    ]) {
      expect(apiClient).not.toHaveProperty(exportName);
    }
  });

  test('previews and imports the exact installed Agent Flow extension', async () => {
    await expect(
      previewConsoleInstalledApplicationExtension(
        'installation-1',
        'https://api.flowbase.test'
      )
    ).resolves.toMatchObject({
      path: '/api/console/applications/archive/installed-extension/installation-1/preview',
      baseUrl: 'https://api.flowbase.test'
    });
    await expect(
      importConsoleInstalledApplicationExtension(
        'installation-1',
        {
          name: 'Imported flow',
          integrity_override: {
            reason: 'user_confirmed',
            acknowledged_warnings: ['checksum_mismatch']
          }
        },
        'csrf-123',
        'https://api.flowbase.test'
      )
    ).resolves.toMatchObject({
      path: '/api/console/applications/archive/installed-extension/installation-1/import',
      method: 'POST',
      body: {
        name: 'Imported flow',
        integrity_override: {
          reason: 'user_confirmed',
          acknowledged_warnings: ['checksum_mismatch']
        }
      },
      csrfToken: 'csrf-123',
      baseUrl: 'https://api.flowbase.test'
    });
  });
});
