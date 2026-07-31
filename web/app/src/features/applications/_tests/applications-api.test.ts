import { afterEach, describe, expect, test, vi } from 'vitest';

vi.mock('@1flowbase/api-client', () => ({
  createConsoleApplication: vi.fn().mockResolvedValue({
    id: 'app-1'
  }),
  createConsoleApplicationTag: vi.fn().mockResolvedValue({
    id: 'tag-1'
  }),
  deleteConsoleApplication: vi.fn().mockResolvedValue(undefined),
  exportConsoleApplicationArchive: vi.fn().mockResolvedValue({
    blob: new Blob(['archive'], { type: 'application/zip' }),
    filename: 'application.zip',
    contentType: 'application/zip'
  }),
  getConsoleApplication: vi.fn().mockResolvedValue({
    id: 'app-1'
  }),
  getConsoleApplicationCatalog: vi.fn().mockResolvedValue({
    types: [],
    tags: []
  }),
  listConsoleApplicationEnvironmentVariables: vi.fn().mockResolvedValue([]),
  listConsoleApplications: vi.fn().mockResolvedValue([]),
  importConsoleApplicationArchive: vi.fn().mockResolvedValue({
    application: { id: 'app-imported' }
  }),
  previewConsoleApplicationArchive: vi.fn().mockResolvedValue({
    unresolved_nodes: []
  }),
  replaceConsoleApplicationEnvironmentVariables: vi.fn().mockResolvedValue([
    {
      name: 'ApiBaseUrl',
      value_type: 'string',
      value: 'https://api.example.com',
      description: '当前应用 API 地址',
      updated_at: '2026-05-09T09:30:00Z'
    }
  ]),
  updateConsoleApplication: vi.fn().mockResolvedValue({
    id: 'app-1'
  }),
  getDefaultApiBaseUrl: vi.fn().mockReturnValue('http://127.0.0.1:7800')
}));

import {
  createConsoleApplication,
  createConsoleApplicationTag,
  deleteConsoleApplication,
  exportConsoleApplicationArchive,
  getConsoleApplication,
  getConsoleApplicationCatalog,
  getDefaultApiBaseUrl,
  importConsoleApplicationArchive,
  listConsoleApplicationEnvironmentVariables,
  listConsoleApplications,
  previewConsoleApplicationArchive,
  replaceConsoleApplicationEnvironmentVariables,
  updateConsoleApplication
} from '@1flowbase/api-client';

import {
  createApplication,
  createApplicationTag,
  deleteApplication,
  exportApplicationArchive,
  fetchApplicationCatalog,
  fetchApplicationDetail,
  fetchApplicationEnvironmentVariables,
  fetchApplications,
  getApplicationsApiBaseUrl,
  importAgentFlowTemplate,
  previewAgentFlowTemplate,
  replaceApplicationEnvironmentVariables,
  updateApplication
} from '../api/applications';

afterEach(() => {
  vi.unstubAllEnvs();
  vi.clearAllMocks();
});

describe('applications api', () => {
  test('applications api prefers VITE_API_BASE_URL when it is present', () => {
    vi.stubEnv('VITE_API_BASE_URL', 'https://api.flowbase.test');

    expect(
      getApplicationsApiBaseUrl({ protocol: 'http:', hostname: 'ignored-host' })
    ).toBe('https://api.flowbase.test');
    expect(getDefaultApiBaseUrl).not.toHaveBeenCalled();
  });

  test('passes the resolved base url to list detail and create requests', async () => {
    const input = {
      application_type: 'agent_flow' as const,
      name: 'Support Agent',
      description: 'customer support',
      icon: 'RobotOutlined',
      icon_type: 'iconfont',
      icon_background: '#E6F7F2'
    };

    expect(
      getApplicationsApiBaseUrl({
        protocol: 'https:',
        hostname: 'workspace.local'
      })
    ).toBe('http://127.0.0.1:7800');

    await fetchApplications();
    await fetchApplicationCatalog();
    await fetchApplicationDetail('app-1');
    await createApplication(input, 'csrf-123');
    await deleteApplication('app-1', 'csrf-123');
    await fetchApplicationEnvironmentVariables('app-1');
    await replaceApplicationEnvironmentVariables(
      'app-1',
      [
        {
          name: 'ApiBaseUrl',
          value_type: 'string',
          value: 'https://api.example.com',
          description: '当前应用 API 地址'
        }
      ],
      'csrf-123'
    );
    await updateApplication(
      'app-1',
      {
        name: 'Support Agent Pro',
        description: 'updated support',
        tag_ids: ['tag-1']
      },
      'csrf-123'
    );
    await createApplicationTag({ name: '客服' }, 'csrf-123');
    const template: Parameters<typeof previewAgentFlowTemplate>[0] = {
      schema_version: '1flowbase.application-template/v1',
      application: {
        application_type: 'agent_flow',
        name: 'Support Agent',
        description: '',
        icon: null,
        icon_type: null,
        icon_background: null
      },
      flow_document: {
        schemaVersion: '1flowbase.flow/v2',
        meta: { flowId: 'flow-1', name: 'Support Agent', description: '', tags: [] },
        graph: { nodes: [], edges: [] },
        editor: { viewport: { x: 0, y: 0, zoom: 1 }, annotations: [], activeContainerPath: [] }
      },
      dependencies: []
    };
    await exportApplicationArchive(['app-1']);
    await previewAgentFlowTemplate(template);
    await importAgentFlowTemplate(
      {
        template,
        name: 'Imported Agent',
        description: ''
      },
      'csrf-123'
    );

    expect(listConsoleApplications).toHaveBeenCalledWith(
      'http://127.0.0.1:7800'
    );
    expect(getConsoleApplicationCatalog).toHaveBeenCalledWith(
      'http://127.0.0.1:7800'
    );
    expect(getConsoleApplication).toHaveBeenCalledWith(
      'app-1',
      'http://127.0.0.1:7800'
    );
    expect(createConsoleApplication).toHaveBeenCalledWith(
      input,
      'csrf-123',
      'http://127.0.0.1:7800'
    );
    expect(deleteConsoleApplication).toHaveBeenCalledWith(
      'app-1',
      'csrf-123',
      'http://127.0.0.1:7800'
    );
    expect(listConsoleApplicationEnvironmentVariables).toHaveBeenCalledWith(
      'app-1',
      'http://127.0.0.1:7800'
    );
    expect(replaceConsoleApplicationEnvironmentVariables).toHaveBeenCalledWith(
      'app-1',
      {
        variables: [
          {
            name: 'ApiBaseUrl',
            value_type: 'string',
            value: 'https://api.example.com',
            description: '当前应用 API 地址'
          }
        ]
      },
      'csrf-123',
      'http://127.0.0.1:7800'
    );
    expect(updateConsoleApplication).toHaveBeenCalledWith(
      'app-1',
      {
        name: 'Support Agent Pro',
        description: 'updated support',
        tag_ids: ['tag-1']
      },
      'csrf-123',
      'http://127.0.0.1:7800'
    );
    expect(createConsoleApplicationTag).toHaveBeenCalledWith(
      { name: '客服' },
      'csrf-123',
      'http://127.0.0.1:7800'
    );
    expect(exportConsoleApplicationArchive).toHaveBeenCalledWith(
      { application_ids: ['app-1'] },
      'http://127.0.0.1:7800'
    );
    expect(previewConsoleApplicationArchive).toHaveBeenCalledWith(
      expect.any(Blob),
      'official-template.json',
      'http://127.0.0.1:7800'
    );
    expect(importConsoleApplicationArchive).toHaveBeenCalledWith(
      expect.objectContaining({
        file: expect.any(Blob),
        filename: 'official-template.json',
        name: 'Imported Agent',
        description: ''
      }),
      'csrf-123',
      'http://127.0.0.1:7800'
    );
    expect(getDefaultApiBaseUrl).toHaveBeenCalled();
  });
});
