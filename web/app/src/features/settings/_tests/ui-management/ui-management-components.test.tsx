import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const router = vi.hoisted(() => ({
  navigate: vi.fn(),
  pathname: '/settings/ui-management/components'
}));

vi.mock('@tanstack/react-router', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@tanstack/react-router')>()),
  useNavigate: () => router.navigate,
  useRouterState: ({ select }: { select: (state: unknown) => unknown }) =>
    select({ location: { pathname: router.pathname } })
}));

const uiManagementApi = vi.hoisted(() => ({
  settingsUiComponentsQueryKey: ['settings', 'ui-management', 'components'],
  settingsUiTemplatesQueryKey: ['settings', 'ui-management', 'templates'],
  fetchSettingsUiComponents: vi.fn(),
  fetchSettingsUiTemplates: vi.fn(),
  updateSettingsUiComponentContract: vi.fn(),
  updateSettingsUiComponentState: vi.fn(),
  archiveSettingsUiTemplate: vi.fn(),
  createSettingsUiTemplate: vi.fn(),
  publishSettingsUiTemplate: vi.fn(),
  resetSettingsUiTemplateDefault: vi.fn(),
  setSettingsUiTemplateDefault: vi.fn(),
  updateSettingsUiTemplate: vi.fn()
}));

vi.mock('../../api/ui-management', () => uiManagementApi);

import { AppProviders } from '../../../../app/AppProviders';
import { UiManagementPanel } from '../../components/ui-management/UiManagementPanel';

describe('UiManagementPanel components', () => {
  beforeEach(() => {
    window.localStorage.removeItem('settings.ui_management.components');
    const structuredContract = {
      component_code: 'data_table',
      export_name: 'DataTable',
      upstream: {
        package: '@1flowbase/native-components',
        component: 'DataTable',
        version: '1.0.0'
      },
      description: 'Displays tabular data.',
      props: [
        {
          name: 'rows',
          type: 'Row[]',
          required: true,
          description: 'Rows to display.'
        }
      ],
      limitations: ['Use serializable data only.'],
      examples: [{ title: 'Basic table', code: '<DataTable rows={[]} />' }],
      insert_snippet: '<DataTable rows={[]} />'
    };
    uiManagementApi.fetchSettingsUiComponents.mockResolvedValue([
      {
        provider_code: 'official-ui',
        contribution_code: 'native-components',
        module_source: '@1flowbase/native-components',
        export_name: 'DataTable',
        module_version: '1.0.0',
        state: 'published',
        official_contract: structuredContract,
        latest_contract: structuredContract,
        published_contract: structuredContract,
        latest_revision: 2,
        published_revision: 2
      },
      {
        provider_code: 'workspace-ui',
        contribution_code: 'custom-components',
        module_source: '@workspace/ui',
        export_name: 'StatusPanel',
        module_version: '0.4.0',
        state: 'hidden',
        official_contract: null,
        latest_contract: {},
        published_contract: null,
        latest_revision: 1,
        published_revision: null
      }
    ]);
  });

  test('AC-001 AC-003 uses the shared management table layout and toolbar', async () => {
    const { container } = render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );

    expect(await screen.findByText('DataTable')).toBeInTheDocument();
    expect(container.querySelector('.data-table-layout')).toBeInTheDocument();
    expect(container.querySelector('.data-table')).toBeInTheDocument();
    expect(container.querySelector('.data-table__scroll-area')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /刷\s*新/ })
    ).toBeInTheDocument();
    expect(screen.getByLabelText('字段配置')).toBeInTheDocument();
  });

  test('AC-002 applies component filters together and resets them', async () => {
    render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );

    expect(await screen.findByText('DataTable')).toBeInTheDocument();
    const search = screen.getByRole('searchbox', {
      name: '搜索组件、模块或所属贡献'
    });
    fireEvent.change(search, { target: { value: 'StatusPanel' } });

    expect(screen.getByText('DataTable')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /筛\s*选/ }));

    await waitFor(() => {
      expect(screen.queryByText('DataTable')).not.toBeInTheDocument();
      expect(screen.getByText('StatusPanel')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: /重\s*置/ }));
    expect(await screen.findByText('DataTable')).toBeInTheDocument();
    expect(search).toHaveValue('');
  });

  test('AC-004 renders unboxed module text and text-only component actions', async () => {
    const { container } = render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );

    expect(await screen.findByText('DataTable')).toBeInTheDocument();
    expect(container.querySelector('code')).toBeNull();

    for (const name of ['编辑', '发布', '隐藏', '恢复官方 Contract']) {
      expect(screen.getAllByRole('button', { name })[0]).toHaveClass(
        'ant-btn-link'
      );
    }
  });

  test('AC-005 separates module source and module version into independent columns', async () => {
    render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );

    expect(await screen.findByText('DataTable')).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '模块来源' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '模块版' })
    ).toBeInTheDocument();
  });

  test('AC-004 edits a component contract through structured fields', async () => {
    render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );

    await screen.findByText('DataTable');
    fireEvent.click(screen.getAllByRole('button', { name: '编辑' })[0]);

    expect(await screen.findByLabelText('组件代码')).toBeInTheDocument();
    expect(screen.getByLabelText('导出名称')).toHaveValue('DataTable');
    expect(screen.getByLabelText('说明')).toBeInTheDocument();
    expect(screen.getByLabelText('插入代码')).toBeInTheDocument();
    expect(screen.queryByLabelText('Contract JSON')).not.toBeInTheDocument();
  });

  test('AC-004 stacks each structured contract entry in one column', async () => {
    render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );

    await screen.findByText('DataTable');
    fireEvent.click(screen.getAllByRole('button', { name: '编辑' })[0]);

    await screen.findByLabelText('组件代码');
    for (const input of [
      screen.getByPlaceholderText('属性名'),
      screen.getByPlaceholderText('示例标题'),
      screen.getByPlaceholderText('包名')
    ]) {
      expect(input.closest('.ant-flex-vertical')).not.toBeNull();
    }
  });
});
