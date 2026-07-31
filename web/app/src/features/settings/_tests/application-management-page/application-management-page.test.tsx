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
  exportApplicationArchive: vi.fn()
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
vi.mock('../../../applications/lib/template-download', () =>
  applicationArchiveDownload
);

import { AppProviders } from '../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { ApplicationManagementPanel } from '../../components/application-management/ApplicationManagementPanel';

describe('ApplicationManagementPanel', () => {
  beforeEach(() => {
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
    fireEvent.click(screen.getByRole('button', { name: /展\s*开/ }));
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

  test('AC-001 AC-002 renders application names and descriptions in separate table columns', async () => {
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    const applicationName = await screen.findByText('Daily Report');
    const applicationDescription = screen.getByText(
      'Generate a report every day'
    );

    expect(
      screen.getByRole('columnheader', { name: '名称' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '简介' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('columnheader', { name: '应用' })
    ).not.toBeInTheDocument();
    expect(applicationName.closest('td')).not.toBe(
      applicationDescription.closest('td')
    );
  });

  test('applies filter drafts together and resets them from the filter form', async () => {
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    await screen.findByText('Daily Report');
    const searchInput = screen.getByRole('searchbox', {
      name: '搜索应用名称或 ID'
    });
    const requestCountBeforeDraftChange =
      applicationManagementApi.fetchSettingsApplicationManagement.mock.calls
        .length;
    fireEvent.change(searchInput, { target: { value: 'Weekly' } });

    expect(
      applicationManagementApi.fetchSettingsApplicationManagement.mock.calls
    ).toHaveLength(requestCountBeforeDraftChange);

    fireEvent.click(screen.getByRole('button', { name: /筛\s*选/ }));

    await waitFor(() => {
      expect(window.location.search).toContain('keyword=Weekly');
      expect(
        applicationManagementApi.fetchSettingsApplicationManagement
      ).toHaveBeenLastCalledWith(
        expect.objectContaining({
          page: 1,
          filter: {
            $and: [
              { application_type: 'workflow' },
              { publication_status: 'unpublished' },
              {
                $or: [
                  { name: { $includes: 'Weekly' } },
                  { id: { $includes: 'Weekly' } }
                ]
              }
            ]
          }
        })
      );
    });

    fireEvent.click(screen.getByRole('button', { name: /重\s*置/ }));

    await waitFor(() => {
      expect(window.location.search).toBe('');
      expect(searchInput).toHaveValue('');
      expect(
        applicationManagementApi.fetchSettingsApplicationManagement
      ).toHaveBeenLastCalledWith({
        page: 1,
        page_size: 20,
        filter: undefined,
        sort: 'updated_at:desc'
      });
    });
  });

  test('AC-001 AC-002 AC-003 collapses secondary filters behind the action-row toggle', async () => {
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    await screen.findByText('Daily Report');
    expect(
      screen.getByRole('combobox', { name: '应用类型' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('searchbox', { name: '搜索应用名称或 ID' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('combobox', { name: '排序' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: '发布状态' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: '创建者' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('combobox', { name: '标签' })
    ).not.toBeInTheDocument();

    const filterActions = document.querySelector(
      '.application-management-panel__filter-actions'
    ) as HTMLElement;
    expect(
      within(filterActions)
        .getAllByRole('button')
        .map((button) => button.textContent?.replaceAll(' ', ''))
    ).toEqual(['展开', '重置', '筛选']);
    const expandButton = screen.getByRole('button', { name: /展\s*开/ });
    expect(expandButton).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(expandButton);

    expect(
      screen.getByRole('combobox', { name: '发布状态' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('combobox', { name: '创建者' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('combobox', { name: '标签' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /收\s*起/ })
    ).toHaveAttribute('aria-expanded', 'true');

    fireEvent.click(screen.getByRole('button', { name: /收\s*起/ }));

    expect(
      screen.queryByRole('combobox', { name: '发布状态' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /展\s*开/ })
    ).toBeInTheDocument();
  });

  test('shows an explicit creator loading failure instead of an empty result', async () => {
    membersApi.fetchSettingsMembers.mockRejectedValueOnce(
      new Error('members unavailable')
    );

    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    await screen.findByText('Daily Report');
    fireEvent.click(screen.getByRole('button', { name: /展\s*开/ }));
    fireEvent.mouseDown(screen.getByRole('combobox', { name: '创建者' }));
    expect(await screen.findByText('创建者加载失败')).toBeInTheDocument();
  });

  test('AC-001 AC-002 exports only selected applications as a ZIP archive', async () => {
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    await screen.findByText('Daily Report');
    const exportButton = screen.getByRole('button', { name: /导出应用（0）/ });
    expect(exportButton).toBeDisabled();
    expect(screen.queryByText('导出筛选结果')).not.toBeInTheDocument();
    expect(screen.queryByText(/CSV/)).not.toBeInTheDocument();

    const rowCheckboxes = screen.getAllByRole('checkbox');
    fireEvent.click(rowCheckboxes[1]);
    const selectedExportButton = screen.getByRole('button', {
      name: /导出应用（1）/
    });
    fireEvent.click(selectedExportButton);

    await waitFor(() =>
      expect(applicationsApi.exportApplicationArchive).toHaveBeenCalledWith(
        ['app-workflow'],
        expect.any(Object)
      )
    );
    await waitFor(() =>
      expect(
        applicationArchiveDownload.downloadApplicationArchive
      ).toHaveBeenCalledWith(
        expect.objectContaining({ contentType: 'application/zip' })
      )
    );

    fireEvent.click(screen.getByRole('button', { name: /筛\s*选/ }));
    expect(
      screen.getByRole('button', { name: /导出应用（0）/ })
    ).toBeDisabled();

    await waitFor(() =>
      expect(screen.getAllByRole('checkbox')).toHaveLength(2)
    );
    fireEvent.click(screen.getAllByRole('checkbox')[1]);
    fireEvent.click(screen.getByTitle('2'));
    await waitFor(() => expect(window.location.search).toContain('page=2'));
    expect(
      screen.getByRole('button', { name: /导出应用（0）/ })
    ).toBeDisabled();

    await waitFor(() =>
      expect(screen.getAllByRole('checkbox')).toHaveLength(2)
    );
    fireEvent.click(screen.getAllByRole('checkbox')[1]);
    fireEvent.click(screen.getByRole('button', { name: /重\s*置/ }));
    expect(
      screen.getByRole('button', { name: /导出应用（0）/ })
    ).toBeDisabled();
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

  test('keeps publication control separate from the streamlined actions', async () => {
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    const row = (await screen.findByText('Daily Report')).closest('tr');
    expect(row).not.toBeNull();

    const headers = screen.getAllByRole('columnheader');
    const publicationControlIndex = headers.findIndex(
      (header) => header.textContent === '发布控制'
    );
    const actionsIndex = headers.findIndex(
      (header) => header.textContent === '操作'
    );

    expect(publicationControlIndex).toBeGreaterThanOrEqual(0);
    expect(actionsIndex).toBeGreaterThanOrEqual(0);

    const publicationControlCell = row?.children.item(publicationControlIndex);
    const actionsCell = row?.children.item(actionsIndex);
    expect(publicationControlCell).not.toBeNull();
    expect(actionsCell).not.toBeNull();
    expect(
      within(publicationControlCell as HTMLElement).getByRole('switch')
    ).toBeInTheDocument();
    expect(
      within(actionsCell as HTMLElement).queryByRole('switch')
    ).not.toBeInTheDocument();

    const editButton = within(actionsCell as HTMLElement).getByRole('button', {
      name: /编\s*辑/
    });
    expect(editButton.querySelector('.anticon')).toBeNull();

    expect(
      within(actionsCell as HTMLElement).getByRole('link', {
        name: /查\s*看/
      })
    ).toHaveAttribute(
      'href',
      '/applications/app-workflow/orchestration'
    );
  });

  test('keeps blank row clicks inert while edit remains an explicit action', async () => {
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    const row = (await screen.findByText('Daily Report')).closest('tr');
    expect(row).not.toBeNull();
    fireEvent.click(row as HTMLElement);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    fireEvent.click(
      within(row as HTMLElement).getByRole('button', { name: /编\s*辑/ })
    );
    expect(
      await screen.findByRole('dialog', { name: '编辑应用信息' })
    ).toBeInTheDocument();
  });

  test('shows newly added publication control for an existing saved column configuration', async () => {
    window.localStorage.setItem(
      'settings.application_management',
      JSON.stringify({
        visibleColumnKeys: [
          'application',
          'tags',
          'created_at',
          'updated_at',
          'actions'
        ],
        columnWidths: {
          application: 260,
          application_type: 120,
          workflow_trigger_type: 120,
          publication_status: 120,
          created_by_display_name: 150,
          tags: 180,
          created_at: 180,
          updated_at: 180,
          actions: 140
        }
      })
    );

    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    expect(
      await screen.findByRole('columnheader', { name: '发布控制' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('columnheader', { name: '应用类型' })
    ).not.toBeInTheDocument();
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

    const row = (await screen.findByText('Order Extension')).closest('tr');
    fireEvent.click(
      within(row as HTMLElement).getByRole('button', { name: /编\s*辑/ })
    );

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

    const row = (await screen.findByText('Daily Report')).closest('tr');
    fireEvent.click(
      within(row as HTMLElement).getByRole('button', { name: /编\s*辑/ })
    );
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
