import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, test } from 'vitest';
import { ApiClientError, AppProviders, ApplicationManagementPanel, applicationManagementApi, applicationsApi, applicationsPublicApi, membersApi, setupApplicationManagementFixture, useAuthStore } from './fixture';

describe('ApplicationManagementPanel', () => {
  setupApplicationManagementFixture();

  test.each(['success', 'partial', 'failure'] as const)(
    'batch delete AC-001 AC-002 AC-004 confirms the selection and handles %s',
    async (outcome) => {
      const initial =
        await applicationManagementApi.fetchSettingsApplicationManagement();
      let items = [
        initial.items[0],
        { ...initial.items[0], id: 'app-second', name: 'Second App' }
      ];
      applicationManagementApi.fetchSettingsApplicationManagement.mockImplementation(
        async () => ({
          ...initial,
          items,
          total: 20 + items.length
        })
      );
      let releaseFirst!: () => void;
      const firstRequest = new Promise<void>((resolve) => {
        releaseFirst = resolve;
      });
      applicationsApi.deleteApplication.mockImplementation(
        async (id: string) => {
          if (id === 'app-workflow') await firstRequest;
          if (
            outcome === 'failure' ||
            (outcome === 'partial' && id === 'app-second')
          ) {
            throw new ApiClientError({
              status: 403,
              code: 'forbidden',
              message: 'internal detail'
            });
          }
          items = items.filter((item) => item.id !== id);
        }
      );
      render(
        <AppProviders>
          <ApplicationManagementPanel />
        </AppProviders>
      );
      await screen.findByText('Second App');
      expect(screen.getByRole('button', { name: '删除（0）' })).toBeDisabled();
      fireEvent.click(screen.getAllByRole('checkbox')[0]);
      fireEvent.click(screen.getByRole('button', { name: '删除（2）' }));
      let dialog = await screen.findByRole('dialog');
      expect(
        within(dialog).getByText('确定删除选中的 2 个应用？此操作不可撤销。')
      ).toBeInTheDocument();
      expect(applicationsApi.deleteApplication).not.toHaveBeenCalled();
      fireEvent.click(within(dialog).getByRole('button', { name: /取\s*消/ }));
      await waitFor(() =>
        expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      );
      expect(screen.getByRole('button', { name: '删除（2）' })).toBeEnabled();
      fireEvent.click(screen.getByRole('button', { name: '删除（2）' }));
      dialog = await screen.findByRole('dialog');
      fireEvent.click(within(dialog).getByRole('button', { name: /删\s*除/ }));
      await waitFor(() =>
        expect(applicationsApi.deleteApplication).toHaveBeenCalledWith(
          'app-workflow',
          'csrf-123'
        )
      );
      expect(applicationsApi.deleteApplication).toHaveBeenCalledTimes(1);
      expect(screen.getByRole('button', { name: '删除（2）' })).toBeDisabled();
      expect(screen.getAllByRole('checkbox')[1]).toBeDisabled();
      releaseFirst();
      const failedCount =
        outcome === 'success' ? 0 : outcome === 'partial' ? 1 : 2;
      await screen.findAllByText(
        `删除成功：${2 - failedCount}，失败：${failedCount}`
      );
      await waitFor(() =>
        expect(
          screen.getByRole('button', { name: `删除（${failedCount}）` })
        ).not.toHaveClass('ant-btn-loading')
      );
      expect(applicationsApi.deleteApplication.mock.calls).toEqual([
        ['app-workflow', 'csrf-123'],
        ['app-second', 'csrf-123']
      ]);
      if (failedCount === 0) {
        expect(screen.queryByText('Daily Report')).not.toBeInTheDocument();
        expect(screen.queryByText('Second App')).not.toBeInTheDocument();
        expect(
          screen.getByRole('button', { name: '删除（0）' })
        ).toBeDisabled();
      } else {
        expect(screen.getAllByRole('checkbox').slice(1)).toHaveLength(
          failedCount
        );
        for (const checkbox of screen.getAllByRole('checkbox').slice(1))
          expect(checkbox).toBeChecked();
        expect(screen.getAllByText(/缺少应用删除权限/)).toHaveLength(
          failedCount
        );
        expect(screen.queryByText('internal detail')).not.toBeInTheDocument();
        // Retrying must submit only the entries that failed previously.
        fireEvent.click(
          within(screen.getByRole('dialog')).getByRole('button', {
            name: /关\s*闭/
          })
        );
        await waitFor(() =>
          expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
        );
        applicationsApi.deleteApplication.mockImplementation(
          async (id: string) => {
            items = items.filter((item) => item.id !== id);
          }
        );
        fireEvent.click(
          screen.getByRole('button', { name: `删除（${failedCount}）` })
        );
        fireEvent.click(
          within(await screen.findByRole('dialog')).getByRole('button', {
            name: /删\s*除/
          })
        );
        await waitFor(() =>
          expect(
            screen.getByRole('button', { name: '删除（0）' })
          ).toBeDisabled()
        );
        await screen.findByText(`删除成功：${failedCount}，失败：0`);
        await waitFor(() =>
          expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
        );
        expect(
          applicationsApi.deleteApplication.mock.calls
            .slice(2)
            .map(([id]) => id)
        ).toEqual(
          outcome === 'partial'
            ? ['app-second']
            : ['app-workflow', 'app-second']
        );
      }
    }
  );

  test.each([
    { permissions: [] },
    { permissions: ['application.delete.own'] },
    { permissions: ['application.delete.all'] }
  ])(
    'batch delete AC-003 respects deletion permissions %j without restricting export selection',
    async ({ permissions }) => {
      const initial =
        await applicationManagementApi.fetchSettingsApplicationManagement();
      applicationManagementApi.fetchSettingsApplicationManagement.mockResolvedValue(
        {
          ...initial,
          items: [
            initial.items[0],
            {
              ...initial.items[0],
              id: 'app-other',
              name: 'Other App',
              created_by: 'other-user'
            }
          ]
        }
      );
      const { actor, me } = useAuthStore.getState();
      useAuthStore.getState().setAuthenticated({
        csrfToken: 'csrf-123',
        actor: { ...actor!, effective_display_role: 'member' },
        me: { ...me!, effective_display_role: 'member', permissions }
      });
      render(
        <AppProviders>
          <ApplicationManagementPanel />
        </AppProviders>
      );
      await screen.findByText('Other App');
      fireEvent.click(screen.getAllByRole('checkbox')[1]);
      if (permissions.length === 0) {
        expect(
          screen.queryByRole('button', { name: /删除（/ })
        ).not.toBeInTheDocument();
      } else {
        expect(screen.getByRole('button', { name: '删除（1）' })).toBeEnabled();
        fireEvent.click(screen.getAllByRole('checkbox')[2]);
        if (permissions.includes('application.delete.all')) {
          expect(
            screen.getByRole('button', { name: '删除（2）' })
          ).toBeEnabled();
        } else {
          expect(
            screen.getByRole('button', { name: '删除（2）' })
          ).toBeDisabled();
        }
      }
      expect(
        screen.getByRole('button', { name: /导出应用（[12]）/ })
      ).toBeEnabled();
      expect(applicationsApi.deleteApplication).not.toHaveBeenCalled();
    }
  );

  test('AC-003 AC-006 restores URL filters and renders backend management fields', async () => {
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    expect(await screen.findByText('Daily Report')).toBeInTheDocument();
    expect(membersApi.fetchSettingsMembers).toHaveBeenCalledWith();
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

  test('AC-001 opens the existing create application form from the management toolbar', async () => {
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    await screen.findByText('Daily Report');
    fireEvent.click(screen.getByRole('button', { name: '新增' }));

    expect(
      await screen.findByRole('dialog', { name: '新建应用' })
    ).toBeInTheDocument();
  });

  test('hides the create action without application creation permission', async () => {
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'member-user',
        account: 'member',
        effective_display_role: 'member',
        current_workspace_id: 'workspace-1'
      },
      me: {
        id: 'member-user',
        account: 'member',
        email: 'member@example.com',
        phone: null,
        nickname: 'Member',
        name: 'Member',
        avatar_url: null,
        introduction: '',
        effective_display_role: 'member',
        permissions: []
      }
    });

    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    await screen.findByText('Daily Report');
    expect(
      screen.queryByRole('button', { name: '新增' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '导入应用' })
    ).not.toBeInTheDocument();
    expect(screen.queryByLabelText('导入')).not.toBeInTheDocument();
  });

  test('log reception uses API status, preserves publication and updates from server state', async () => {
    window.history.replaceState({}, '', '/settings/applications');
    const row = {
      id: 'logs',
      application_type: 'agent_logs',
      name: 'Reception logs',
      created_by: 'root-user',
      created_by_display_name: 'Root',
      tags: [],
      created_at: '2026-10-11T00:00:00Z',
      updated_at: '2026-10-11T00:00:00Z',
      publication_status: 'not_applicable',
      api_enabled: false
    };
    applicationManagementApi.fetchSettingsApplicationManagement.mockResolvedValue(
      { items: [row], total: 1, page: 1, page_size: 20 }
    );
    applicationsPublicApi.updateApplicationApiStatus.mockImplementation(
      async () => {
        row.api_enabled = true;
        return {
          application_id: 'logs',
          api_enabled: true,
          public_url: '/api/logs/v1/events'
        };
      }
    );
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );
    const toggle = await screen.findByRole('switch', { name: '日志接收' });
    expect(toggle).not.toBeChecked();
    expect(screen.getByText('已停用')).toBeInTheDocument();
    fireEvent.click(toggle);
    await waitFor(() =>
      expect(
        applicationsPublicApi.updateApplicationApiStatus
      ).toHaveBeenCalledWith('logs', true, 'csrf-123')
    );
    await waitFor(() =>
      expect(screen.getByRole('switch', { name: '日志接收' })).toBeChecked()
    );
    expect(screen.getByText('接收中')).toBeInTheDocument();
    expect(
      applicationsPublicApi.fetchApplicationApiMapping
    ).not.toHaveBeenCalled();
    expect(
      applicationsPublicApi.unpublishApplicationApiVersion
    ).not.toHaveBeenCalled();
  });

  test('restores agent_logs URL filters and renders backend type labels', async () => {
    applicationManagementApi.fetchSettingsApplicationManagement.mockResolvedValue(
      {
        items: [
          {
            id: 'app-logs',
            application_type: 'agent_logs',
            workflow_trigger_type: null,
            name: 'Collected Logs',
            description: 'Imported application records',
            icon: null,
            icon_type: null,
            icon_background: null,
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
    window.history.replaceState(
      {},
      '',
      '/settings/applications?application_type=agent_logs'
    );
    render(
      <AppProviders>
        <ApplicationManagementPanel />
      </AppProviders>
    );

    const logsRow = await screen.findByRole('row', { name: /Collected Logs/ });
    expect(await within(logsRow).findByText('Agent Logs')).toBeInTheDocument();
    expect(within(logsRow).queryByText('Workflow')).not.toBeInTheDocument();
    await waitFor(() => {
      expect(
        applicationManagementApi.fetchSettingsApplicationManagement
      ).toHaveBeenLastCalledWith({
        page: 1,
        page_size: 20,
        filter: { $and: [{ application_type: 'agent_logs' }] },
        sort: 'updated_at:desc'
      });
    });

    fireEvent.click(screen.getByRole('button', { name: /筛\s*选/ }));
    expect(
      new URLSearchParams(window.location.search).get('application_type')
    ).toBe('agent_logs');
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
    expect(screen.getByRole('combobox', { name: '排序' })).toBeInTheDocument();
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
      '.data-table-filter-form__actions'
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
    expect(screen.getByRole('combobox', { name: '标签' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /收\s*起/ })).toHaveAttribute(
      'aria-expanded',
      'true'
    );

    fireEvent.click(screen.getByRole('button', { name: /收\s*起/ }));

    expect(
      screen.queryByRole('combobox', { name: '发布状态' })
    ).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /展\s*开/ })).toBeInTheDocument();
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
      (header) => header.textContent === '发布／接收控制'
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
    ).toHaveAttribute('href', '/applications/app-workflow/orchestration');
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
      await screen.findByRole('columnheader', { name: '发布／接收控制' })
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
