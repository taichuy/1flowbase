import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  downloadConsoleOfficialAgentFlowTemplate,
  exportConsoleApplicationArchive,
  importConsoleApplicationArchive,
  importConsoleInstalledApplicationExtension,
  listConsoleOfficialAgentFlowTemplateCatalog,
  previewConsoleApplicationArchive,
  previewConsoleInstalledApplicationExtension
} from '../console/application-orchestration';

describe('console application orchestration official template client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );
  vi.spyOn(transport, 'apiFetchBlob').mockImplementation(
    async (input) => input as never
  );

  test('exports selected applications through the ZIP archive route', async () => {
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

  test('points official template catalog at the paged backend route', async () => {
    await expect(
      listConsoleOfficialAgentFlowTemplateCatalog(
        { cursor: '2' },
        'https://api.flowbase.test'
      )
    ).resolves.toMatchObject({
      path: '/api/console/applications/orchestration/templates/official-catalog?cursor=2',
      baseUrl: 'https://api.flowbase.test'
    });
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

  test('downloads official templates through the backend route', async () => {
    await expect(
      downloadConsoleOfficialAgentFlowTemplate(
        'customer/support bot',
        'https://api.flowbase.test'
      )
    ).resolves.toMatchObject({
      path: '/api/console/applications/orchestration/templates/official/customer%2Fsupport%20bot',
      baseUrl: 'https://api.flowbase.test'
    });
  });
});
