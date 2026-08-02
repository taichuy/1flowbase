import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  deleteConsoleOfficialAgentFlowTemplateRelease,
  exportConsoleApplicationArchive,
  importConsoleOfficialAgentFlowTemplate,
  importConsoleApplicationArchive,
  importConsoleInstalledApplicationExtension,
  listConsoleOfficialAgentFlowTemplateCatalog,
  previewConsoleOfficialAgentFlowTemplate,
  previewConsoleApplicationArchive,
  previewConsoleInstalledApplicationExtension,
  repairConsoleOfficialAgentFlowTemplateRelease,
  switchConsoleOfficialAgentFlowTemplateCurrent,
  syncConsoleOfficialAgentFlowTemplate
} from '../console/application-orchestration';

describe('console application orchestration official template client', () => {
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

  test('points official template catalog at the combined local library route', async () => {
    await expect(
      listConsoleOfficialAgentFlowTemplateCatalog('https://api.flowbase.test')
    ).resolves.toMatchObject({
      path: '/api/console/applications/orchestration/templates/official-catalog',
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

  test('uses local-library routes for official template operations', async () => {
    await expect(
      syncConsoleOfficialAgentFlowTemplate(
        'customer/support bot',
        { release_version: 4 },
        'csrf-123',
        'https://api.flowbase.test'
      )
    ).resolves.toMatchObject({
      path: '/api/console/applications/orchestration/templates/official/customer%2Fsupport%20bot/sync',
      method: 'POST',
      body: { release_version: 4 },
      csrfToken: 'csrf-123',
      baseUrl: 'https://api.flowbase.test'
    });
    await expect(
      previewConsoleOfficialAgentFlowTemplate(
        'customer/support bot',
        {},
        'csrf-123',
        'https://api.flowbase.test'
      )
    ).resolves.toMatchObject({
      path: '/api/console/applications/orchestration/templates/official/customer%2Fsupport%20bot/preview',
      method: 'POST',
      body: {},
      csrfToken: 'csrf-123'
    });
    await expect(
      importConsoleOfficialAgentFlowTemplate(
        'customer/support bot',
        { release_version: 3, name: 'Imported flow', description: 'Local' },
        'csrf-123',
        'https://api.flowbase.test'
      )
    ).resolves.toMatchObject({
      path: '/api/console/applications/orchestration/templates/official/customer%2Fsupport%20bot/import',
      method: 'POST',
      body: {
        release_version: 3,
        name: 'Imported flow',
        description: 'Local'
      },
      csrfToken: 'csrf-123'
    });
    await expect(
      switchConsoleOfficialAgentFlowTemplateCurrent(
        'flow',
        2,
        'csrf-123',
        'https://api.flowbase.test'
      )
    ).resolves.toMatchObject({
      path: '/api/console/applications/orchestration/templates/official/flow/current/2',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
    await expect(
      deleteConsoleOfficialAgentFlowTemplateRelease(
        'flow',
        2,
        'csrf-123',
        'https://api.flowbase.test'
      )
    ).resolves.toMatchObject({
      path: '/api/console/applications/orchestration/templates/official/flow/releases/2',
      method: 'DELETE',
      csrfToken: 'csrf-123'
    });
    await expect(
      repairConsoleOfficialAgentFlowTemplateRelease(
        'flow',
        2,
        'csrf-123',
        'https://api.flowbase.test'
      )
    ).resolves.toMatchObject({
      path: '/api/console/applications/orchestration/templates/official/flow/releases/2/repair',
      method: 'POST',
      csrfToken: 'csrf-123'
    });
  });
});
