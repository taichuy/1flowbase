import { render, screen, waitFor, within } from '@testing-library/react';
import { fireEvent } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

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
  exportAgentFlowTemplate: vi.fn()
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
vi.mock('../../api/members', () => membersApi);
vi.mock('../../../agent-flow/api/orchestration', () => orchestrationApi);

import { AppProviders } from '../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { ApplicationManagementPanel } from '../../components/application-management/ApplicationManagementPanel';

describe('ApplicationManagementPanel', () => {
  beforeEach(() => {
    resetAuthStore();
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
      types: [
        { value: 'agent_flow', label: 'AgentFlow' },
        { value: 'workflow', label: 'Workflow' }
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
    applicationsApi.updateApplication.mockResolvedValue({ id: 'app-workflow' });
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

  test('AC-003 AC-006 restores URL filters and renders backend management fields', async () => {
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    expect(await screen.findByText('Daily Report')).toBeInTheDocument();
    expect(
      screen.queryByRole('heading', { name: '应用管理' })
    ).not.toBeInTheDocument();
    expect(screen.getByText('Generate a report every day')).toBeInTheDocument();
    expect(screen.getAllByText('Workflow')).toHaveLength(2);
    expect(screen.getByText('定时任务')).toBeInTheDocument();
    expect(screen.getAllByText('未发布')).toHaveLength(2);
    expect(screen.getByText('Root')).toBeInTheDocument();
    expect(screen.getByText('报表')).toBeInTheDocument();
    expect(screen.getByText('Daily Report')).toBeInTheDocument();

    await waitFor(() => {
      expect(
        applicationManagementApi.fetchSettingsApplicationManagement
      ).toHaveBeenCalledWith({
        page: 2,
        page_size: 20,
        filter: {
          $and: [
            { application_type: 'workflow' },
            { publication_status: 'unpublished' },
            {
              $or: [
                { name: { $includes: 'Daily' } },
                { id: { $includes: 'Daily' } }
              ]
            }
          ]
        },
        sort: 'updated_at:desc'
      });
    });
  });

  test('#1286 AC-002 reverts a published application via the inline switch', async () => {
    applicationManagementApi.fetchSettingsApplicationManagement.mockResolvedValue(
      {
        items: [
          {
            id: 'app-published',
            application_type: 'agent_flow',
            workflow_trigger_type: null,
            name: 'Live Assistant',
            description: 'Serving traffic',
            icon: null,
            icon_type: null,
            icon_background: null,
            created_by: 'root-user',
            created_by_display_name: 'Root',
            created_at: '2026-07-12T08:00:00Z',
            updated_at: '2026-07-13T08:00:00Z',
            tags: [],
            publication_status: 'published'
          }
        ],
        total: 1,
        page: 1,
        page_size: 20
      }
    );

    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    fireEvent.click(await screen.findByRole('switch'));

    const dialog = await screen.findByRole('dialog');
    fireEvent.click(within(dialog).getByRole('button', { name: '退回草稿' }));

    await waitFor(() => {
      expect(
        applicationsPublicApi.unpublishApplicationApiVersion
      ).toHaveBeenCalledWith('app-published', 'csrf-123');
    });
  });

  test('AC-001 AC-008 opens the shared edit modal and keeps extension registration read-only', async () => {
    applicationsApi.fetchApplicationDetail.mockResolvedValue({
      id: 'app-extension',
      application_type: 'workflow',
      workflow_trigger_type: 'extension',
      name: 'Order Extension',
      description: 'Creates an order',
      icon: 'ApiOutlined',
      icon_type: 'iconfont',
      icon_background: '#E6F7F2',
      created_by: 'root-user',
      created_at: '2026-07-12T08:00:00Z',
      updated_at: '2026-07-13T08:00:00Z',
      tags: []
    });
    applicationManagementApi.fetchSettingsApplicationManagement.mockResolvedValue(
      {
        items: [
          {
            id: 'app-extension',
            application_type: 'workflow',
            workflow_trigger_type: 'extension',
            name: 'Order Extension',
            description: 'Creates an order',
            icon: 'ApiOutlined',
            icon_type: 'iconfont',
            icon_background: '#E6F7F2',
            created_by: 'root-user',
            created_by_display_name: 'Root',
            created_at: '2026-07-12T08:00:00Z',
            updated_at: '2026-07-13T08:00:00Z',
            tags: [],
            publication_status: 'unpublished'
          }
        ],
        total: 1,
        page: 1,
        page_size: 20
      }
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
      extension: {
        slug: 'orders/create',
        method: 'POST',
        response_mode: 'sync',
        parameters: []
      }
    });

    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    const name = await screen.findByText('Order Extension');
    fireEvent.click(name.closest('tr') as HTMLElement);

    const drawer = await screen.findByRole('dialog', {
      name: '编辑应用信息'
    });
    expect(document.querySelector('.ant-modal-content')).not.toBeNull();
    expect(document.querySelector('.ant-drawer')).toBeNull();
    expect(document.querySelector('.ant-descriptions')).toBeNull();
    expect(
      await within(drawer).findByDisplayValue('Order Extension')
    ).toBeEnabled();
    await within(drawer).findByDisplayValue('POST');
    expect(
      within(drawer).getByRole('textbox', { name: '请求方式' })
    ).toHaveValue('POST');
    expect(
      within(drawer).getByRole('textbox', { name: '接口子路径' })
    ).toHaveValue('/api/ex/orders/create');
    expect(
      within(drawer).getByRole('textbox', { name: '响应方式' })
    ).toHaveValue('同步');
    expect(
      within(drawer).getByRole('textbox', { name: '触发方式' })
    ).toHaveValue('扩展接口触发');
    expect(
      within(drawer).getByRole('textbox', { name: '请求参数' })
    ).toHaveValue('body · customer_id · string');
    expect(
      within(drawer).getByRole('textbox', { name: '响应字段' })
    ).toHaveValue('order_id · string');
    expect(
      within(drawer).queryByRole('textbox', { name: '接口路径' })
    ).not.toBeInTheDocument();
  });

  test('AC-001 saves schedule configuration through its dedicated endpoint', async () => {
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    const name = await screen.findByText('Daily Report');
    fireEvent.click(name.closest('tr') as HTMLElement);
    const drawer = await screen.findByRole('dialog', {
      name: '编辑应用信息'
    });
    const cron = await within(drawer).findByDisplayValue('0 9 * * 1-5');
    const customerId = await within(drawer).findByRole('textbox', {
      name: 'Customer ID · string'
    });
    expect(
      within(drawer).queryByText(/body · customer_id/)
    ).not.toBeInTheDocument();
    fireEvent.change(customerId, { target: { value: 'C-42' } });
    fireEvent.change(cron, { target: { value: '0 10 * * 1-5' } });
    fireEvent.click(within(drawer).getByRole('button', { name: '保存修改' }));

    await waitFor(() => {
      expect(
        applicationsPublicApi.saveWorkflowScheduleTrigger
      ).toHaveBeenCalledWith(
        'app-workflow',
        {
          enabled: false,
          cron: '0 10 * * 1-5',
          timezone: 'Asia/Shanghai',
          input_payload: { customer_id: 'C-42' }
        },
        'csrf-123'
      );
    });
  });
});
