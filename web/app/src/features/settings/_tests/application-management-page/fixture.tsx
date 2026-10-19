import { App } from 'antd';
import type { CSSProperties, ReactNode } from 'react';
import { beforeAll, beforeEach, vi } from 'vitest';

vi.mock('../../../../shared/ui/fixed-height-modal/FixedHeightModal', () => ({
  FixedHeightModal: ({
    children,
    className,
    footer,
    open,
    scrollBodyClassName,
    title,
    width
  }: {
    children: ReactNode;
    className?: string;
    footer?: ReactNode;
    open: boolean;
    scrollBodyClassName?: string;
    title: ReactNode;
    width?: CSSProperties['width'];
  }) =>
    open ? (
      <dialog
        aria-label={String(title)}
        className={`ant-modal ${className ?? ''}`}
        open
        style={{ width }}
      >
        <div className="ant-modal-content">
          <div
            className={scrollBodyClassName}
            data-testid="fixed-height-modal-scroll-body"
          >
            {children}
          </div>
          {footer}
        </div>
      </dialog>
    ) : null
}));

const applicationManagementApi = vi.hoisted(() => ({
  settingsApplicationManagementQueryPrefix: ['settings', 'applications'],
  settingsApplicationManagementQueryKey: vi.fn((query: unknown) => [
    'settings',
    'applications',
    query
  ]),
  fetchSettingsApplicationManagement: vi.fn()
}));

const applicationsApi = vi.hoisted(() => ({
  applicationCatalogQueryKey: ['applications', 'catalog'],
  applicationDetailQueryKey: vi.fn((applicationId: string) => [
    'applications',
    applicationId
  ]),
  applicationsQueryKey: ['applications'],
  fetchApplicationCatalog: vi.fn(),
  fetchApplicationDetail: vi.fn(),
  updateApplication: vi.fn(),
  deleteApplication: vi.fn(),
  createApplication: vi.fn(),
  createApplicationTag: vi.fn(),
  exportApplicationArchive: vi.fn(),
  previewApplicationArchive: vi.fn(),
  importApplicationArchive: vi.fn()
}));

const applicationArchiveDownload = vi.hoisted(() => ({
  downloadApplicationArchive: vi.fn()
}));

const applicationsPublicApi = vi.hoisted(() => ({
  applicationApiMappingQueryKey: vi.fn((applicationId: string) => [
    'applications',
    applicationId,
    'public-api',
    'mapping'
  ]),
  workflowScheduleTriggerQueryKey: vi.fn((applicationId: string) => [
    'applications',
    applicationId,
    'workflow',
    'schedule-trigger'
  ]),
  unpublishApplicationApiVersion: vi.fn(),
  updateApplicationApiStatus: vi.fn(),
  fetchApplicationApiMapping: vi.fn(),
  fetchWorkflowScheduleTrigger: vi.fn(),
  saveWorkflowScheduleTrigger: vi.fn()
}));

const membersApi = vi.hoisted(() => ({
  settingsMembersQueryKey: ['settings', 'members'],
  fetchSettingsMembers: vi.fn()
}));

const orchestrationApi = vi.hoisted(() => ({
  orchestrationQueryKey: vi.fn((applicationId: string) => [
    'applications',
    applicationId,
    'orchestration'
  ]),
  fetchOrchestrationState: vi.fn()
}));

vi.mock('../../api/application-management', () => applicationManagementApi);
vi.mock('../../../applications/api/applications', async (importOriginal) => ({
  ...(await importOriginal<
    typeof import('../../../applications/api/applications')
  >()),
  ...applicationsApi
}));
vi.mock('../../../applications/api/public-api', () => applicationsPublicApi);
vi.mock('../../api/identity-access/members', () => membersApi);
vi.mock('../../../agent-flow/api/orchestration', () => orchestrationApi);
vi.mock(
  '../../../applications/lib/template-download',
  () => applicationArchiveDownload
);

import { loadApplicationI18nResources } from '../../../../shared/i18n/app-i18n';

import { AppProviders as BaseAppProviders } from '../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';

export function AppProviders({ children }: { children: ReactNode }) {
  return (
    <BaseAppProviders>
      <App>{children}</App>
    </BaseAppProviders>
  );
}

export function setupApplicationManagementFixture() {
  beforeAll(() => loadApplicationI18nResources());
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
    window.localStorage.removeItem('settings.application_management');
    window.history.replaceState(
      {},
      '',
      '/settings/applications?page=2&application_type=workflow&publication_status=unpublished&keyword=Daily'
    );
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'root-user',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      me: {
        id: 'root-user',
        account: 'root',
        email: 'root@example.com',
        phone: null,
        nickname: 'Root',
        name: 'Root',
        avatar_url: null,
        introduction: '',
        effective_display_role: 'root',
        permissions: []
      }
    });
    applicationManagementApi.fetchSettingsApplicationManagement.mockResolvedValue(
      {
        items: [
          {
            id: 'app-workflow',
            application_type: 'workflow',
            workflow_trigger_type: 'schedule',
            name: 'Daily Report',
            description: 'Generate a report every day',
            icon: null,
            icon_type: null,
            icon_background: null,
            created_by: 'root-user',
            created_by_display_name: 'Root',
            created_at: '2026-07-12T08:00:00Z',
            updated_at: '2026-07-13T08:00:00Z',
            tags: [{ id: 'tag-report', name: '报表' }],
            publication_status: 'unpublished'
          }
        ],
        total: 21,
        page: 2,
        page_size: 20
      }
    );
    applicationsApi.fetchApplicationCatalog.mockResolvedValue({
      collectors: [],
      types: [
        {
          value: 'agent_flow',
          label: 'AgentFlow'
        },
        {
          value: 'workflow',
          label: 'Workflow'
        },
        {
          value: 'agent_logs',
          label: 'Agent Logs'
        }
      ],
      workflow_triggers: [
        {
          value: 'extension',
          label: '扩展接口触发'
        },
        {
          value: 'schedule',
          label: '定时触发'
        }
      ],
      tags: [{ id: 'tag-report', name: '报表', application_count: 1 }]
    });
    applicationsApi.fetchApplicationDetail.mockResolvedValue({
      id: 'app-workflow',
      application_type: 'workflow',
      workflow_trigger_type: 'schedule',
      name: 'Daily Report',
      description: 'Generate a report every day',
      icon: null,
      icon_type: null,
      icon_background: null,
      created_by: 'root-user',
      created_at: '2026-07-12T08:00:00Z',
      updated_at: '2026-07-13T08:00:00Z',
      tags: [{ id: 'tag-report', name: '报表' }]
    });
    applicationsApi.previewApplicationArchive.mockResolvedValue({
      applications: [
        {
          entry_index: 0,
          preview: {
            application: {
              name: 'Imported Report',
              description: 'Imported description',
              application_type: 'workflow'
            },
            dependencies: [],
            unresolved_nodes: []
          }
        }
      ]
    });
    applicationsApi.importApplicationArchive.mockResolvedValue({
      succeeded_count: 1,
      failed_count: 0,
      partial_count: 0,
      results: [
        {
          entry_index: 0,
          status: 'succeeded',
          result: { application: { id: 'app-imported' } }
        }
      ]
    });
    applicationsApi.updateApplication.mockResolvedValue({ id: 'app-workflow' });
    applicationsApi.exportApplicationArchive.mockResolvedValue({
      blob: new Blob(['archive'], { type: 'application/zip' }),
      filename: 'applications-2026-07-31.zip',
      contentType: 'application/zip'
    });
    applicationsPublicApi.unpublishApplicationApiVersion.mockResolvedValue(
      undefined
    );
    applicationsPublicApi.fetchApplicationApiMapping.mockResolvedValue({
      input: {
        query_target: 'node-start.query',
        model_target: null,
        inputs_target: 'node-start',
        history_target: null,
        attachments_target: null
      },
      output: {
        answer_selector: null,
        usage_selector: null,
        files_selector: null,
        error_selector: null
      },
      extension: null
    });
    applicationsPublicApi.fetchWorkflowScheduleTrigger.mockResolvedValue({
      application_id: 'app-workflow',
      enabled: false,
      cron: '0 9 * * 1-5',
      timezone: 'Asia/Shanghai',
      input_payload: {}
    });
    applicationsPublicApi.saveWorkflowScheduleTrigger.mockResolvedValue({
      application_id: 'app-workflow',
      enabled: false,
      cron: '0 9 * * 1-5',
      timezone: 'Asia/Shanghai',
      input_payload: {}
    });
    membersApi.fetchSettingsMembers.mockResolvedValue([
      {
        id: 'root-user',
        account: 'root',
        email: 'root@example.com',
        phone: null,
        name: 'Root',
        nickname: 'Root',
        introduction: '',
        default_display_role: 'root',
        email_login_enabled: true,
        phone_login_enabled: false,
        status: 'active',
        role_codes: ['root']
      }
    ]);
    orchestrationApi.fetchOrchestrationState.mockResolvedValue({
      flow_id: 'flow-extension',
      messages: [],
      draft: {
        id: 'draft-extension',
        flow_id: 'flow-extension',
        updated_at: '2026-07-18T08:00:00Z',
        document: {
          schemaVersion: '1.0.0',
          meta: {
            flowId: 'flow-extension',
            name: '',
            description: '',
            tags: []
          },
          graph: {
            nodes: [
              {
                id: 'node-workflow-start',
                type: 'workflow_start',
                alias: 'Workflow Start',
                containerId: null,
                position: { x: 0, y: 0 },
                configVersion: 1,
                config: {
                  input_fields: [
                    {
                      key: 'customer_id',
                      label: 'Customer ID',
                      inputType: 'text',
                      valueType: 'string',
                      required: true,
                      source: 'body'
                    }
                  ]
                },
                bindings: {},
                outputs: []
              },
              {
                id: 'node-workflow-end',
                type: 'workflow_end',
                alias: 'Workflow End',
                containerId: null,
                position: { x: 0, y: 0 },
                configVersion: 1,
                config: {},
                bindings: {},
                outputs: [
                  { key: 'order_id', title: 'Order ID', valueType: 'string' }
                ]
              }
            ],
            edges: []
          },
          variables: { conversation: [] },
          editor: {
            viewport: { x: 0, y: 0, zoom: 1 },
            annotations: [],
            activeContainerPath: []
          }
        }
      },
      versions: [],
      autosave_interval_seconds: 10,
      user_protection_limit: 10
    });
  });

}

export {
  applicationManagementApi,
  applicationsApi,
  applicationArchiveDownload,
  applicationsPublicApi,
  membersApi
};
