import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
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

vi.mock('@monaco-editor/react', () => ({
  default: ({
    options,
    value,
    onChange
  }: {
    options?: { ariaLabel?: string; readOnly?: boolean };
    value?: string;
    onChange?: (value: string) => void;
  }) => (
    <textarea
      aria-label={options?.ariaLabel}
      data-read-only={options?.readOnly}
      data-testid="block-source-editor"
      readOnly={options?.readOnly}
      value={value}
      onChange={(event) => onChange?.(event.target.value)}
    />
  )
}));

const uiManagementApi = vi.hoisted(() => ({
  settingsUiComponentsQueryKey: ['settings', 'ui-management', 'components'],
  settingsUiTemplatesQueryKey: ['settings', 'ui-management', 'templates'],
  fetchSettingsUiComponents: vi.fn(),
  fetchSettingsUiComponent: vi.fn(),
  createSettingsUiComponent: vi.fn(),
  updateSettingsUiComponent: vi.fn(),
  deleteSettingsUiComponent: vi.fn(),
  fetchSettingsUiTemplates: vi.fn(),
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
import { useAuthStore } from '../../../../state/auth-store';

const official = {
  id: '018f0000-0000-7000-8000-000000000001',
  scope_id: '00000000-0000-0000-0000-000000000000',
  component_code: 'taichuy.ant-design-x.bubble',
  name: 'Bubble',
  description: 'Conversation bubble',
  import_code: "import { Bubble } from '@ant-design/x';",
  source_code: '<Bubble content="Hello" />',
  origin: 'official' as const,
  source: 'taichuy',
  group: 'ant-design-x',
  upstream: { identity: '@ant-design/x', version: '2.9.0' },
  version: '1.0.0',
  keywords: ['chat'],
  created_at: '2026-08-23T00:00:00Z',
  updated_at: '2026-08-23T00:00:00Z'
};

const custom = {
  ...official,
  id: '018f0000-0000-7000-8000-000000000002',
  component_code: 'local.status-panel',
  name: 'Status panel',
  description: 'System status summary',
  origin: 'custom' as const,
  source: 'local',
  group: 'operations',
  upstream: { identity: '@local/status-panel', version: '0.2.0' }
};

describe('UiManagementPanel component records', () => {
  beforeEach(() => {
    window.localStorage.removeItem('settings.ui_management.components');
    useAuthStore.setState({ csrfToken: 'csrf' });
    vi.clearAllMocks();
    uiManagementApi.fetchSettingsUiComponents.mockResolvedValue([
      official,
      custom
    ]);
    uiManagementApi.fetchSettingsUiComponent.mockImplementation(
      async (id: string) => (id === official.id ? official : custom)
    );
    uiManagementApi.createSettingsUiComponent.mockResolvedValue(custom);
    uiManagementApi.updateSettingsUiComponent.mockResolvedValue(custom);
    uiManagementApi.deleteSettingsUiComponent.mockResolvedValue(undefined);
  });

  test('WP-D2 lists and searches independent persisted records', async () => {
    render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );
    expect(await screen.findByText('Bubble')).toBeInTheDocument();
    expect(screen.getByText('Status panel')).toBeInTheDocument();
    fireEvent.change(screen.getByRole('searchbox', { name: '搜索组件记录' }), {
      target: { value: 'status' }
    });
    fireEvent.click(screen.getByRole('button', { name: /筛\s*选/ }));
    await waitFor(() =>
      expect(screen.queryByText('Bubble')).not.toBeInTheDocument()
    );
    expect(screen.getByText('Status panel')).toBeInTheDocument();
  });

  test('WP-D2 opens official details read-only and exposes custom actions only', async () => {
    render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );
    await screen.findByText('Bubble');
    fireEvent.click(screen.getByRole('button', { name: '查看 Bubble' }));
    const drawer = await screen.findByRole('dialog');
    expect(within(drawer).getByText('官方组件（只读）')).toBeInTheDocument();
    expect(
      within(drawer).queryByRole('button', { name: /保存/ })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '编辑 Bubble' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '编辑 Status panel' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '删除 Status panel' })
    ).toBeInTheDocument();
  });

  test('WP-D2 creates a custom record while keeping code fields opaque', async () => {
    render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );
    await screen.findByText('Bubble');
    fireEvent.click(screen.getByRole('button', { name: '新建组件' }));
    fireEvent.change(await screen.findByLabelText('组件编码'), {
      target: { value: 'local.card' }
    });
    fireEvent.change(screen.getByLabelText('名称'), {
      target: { value: 'Card' }
    });
    fireEvent.change(screen.getByLabelText('说明'), {
      target: { value: 'Card example' }
    });
    fireEvent.change(screen.getByLabelText('来源'), {
      target: { value: 'local' }
    });
    fireEvent.change(screen.getByLabelText('分组'), {
      target: { value: 'layout' }
    });
    fireEvent.change(screen.getByLabelText('上游标识'), {
      target: { value: '@local/card' }
    });
    fireEvent.change(screen.getByLabelText('上游版本'), {
      target: { value: '0.1.0' }
    });
    fireEvent.change(screen.getByLabelText('记录版本'), {
      target: { value: '1.0.0' }
    });
    const editors = screen.getAllByTestId('block-source-editor');
    fireEvent.change(screen.getByRole('textbox', { name: '导入代码' }), {
      target: { value: 'opaque import {{{' }
    });
    fireEvent.change(screen.getByRole('textbox', { name: '源码' }), {
      target: { value: 'opaque source }}}' }
    });
    fireEvent.change(screen.getByLabelText('关键词'), {
      target: { value: 'layout,card' }
    });
    fireEvent.click(screen.getByRole('button', { name: '保存组件' }));
    await waitFor(() =>
      expect(uiManagementApi.createSettingsUiComponent).toHaveBeenCalledWith(
        expect.objectContaining({
          component_code: 'local.card',
          keywords: ['layout', 'card']
        }),
        expect.any(String)
      )
    );
    expect(editors).toHaveLength(2);
  });

  test('WP-D2 updates and deletes custom records by stable id', async () => {
    render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );
    await screen.findByText('Status panel');
    fireEvent.click(screen.getByRole('button', { name: '编辑 Status panel' }));
    const name = await screen.findByLabelText('名称');
    fireEvent.change(name, { target: { value: 'System status panel' } });
    fireEvent.click(screen.getByRole('button', { name: '保存组件' }));
    await waitFor(() =>
      expect(uiManagementApi.updateSettingsUiComponent).toHaveBeenCalledWith(
        custom.id,
        expect.objectContaining({ name: 'System status panel' }),
        expect.any(String)
      )
    );

    fireEvent.click(screen.getByRole('button', { name: '删除 Status panel' }));
    fireEvent.click(await screen.findByRole('button', { name: '确认删除' }));
    await waitFor(() =>
      expect(uiManagementApi.deleteSettingsUiComponent).toHaveBeenCalledWith(
        custom.id,
        expect.any(String)
      )
    );
  });
});
