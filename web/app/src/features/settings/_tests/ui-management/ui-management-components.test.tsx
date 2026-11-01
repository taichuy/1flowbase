import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeAll, beforeEach, describe, expect, test, vi } from 'vitest';

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

vi.mock('../../../../shared/code-block/monaco-runtime', () => ({
  loadMonacoEditorModule: () => import('@monaco-editor/react')
}));
vi.mock('@monaco-editor/react', async () => {
  const { useEffect, useRef, useState } = await import('react');
  return {
    default: function MockMonacoEditor({
      beforeMount,
      onMount,
      options,
      path,
      value,
      onChange
    }: {
      beforeMount?: (monaco: unknown) => void;
      onMount?: (editor: unknown, monaco: unknown) => void;
      options?: { ariaLabel?: string; readOnly?: boolean };
      path?: string;
      value?: string;
      onChange?: (value: string) => void;
    }) {
      const pathRef = useRef(path);
      useEffect(() => {
        pathRef.current = path;
      }, [path]);
      const [mount] = useState(() => {
        const monaco = {
          MarkerSeverity: { Error: 8 },
          editor: { setModelMarkers: vi.fn() },
          languages: {
            typescript: {
              JsxEmit: { Preserve: 'preserve' },
              ModuleResolutionKind: { NodeJs: 'node-js' },
              ScriptTarget: { ES2022: 'es2022' },
              typescriptDefaults: {
                addExtraLib: vi.fn(() => ({ dispose: vi.fn() })),
                setCompilerOptions: vi.fn()
              }
            }
          }
        };
        const editor = {
          getModel: () => ({
            uri: { toString: () => pathRef.current ?? 'file:///source.tsx' }
          })
        };
        return { beforeMount, onMount, editor, monaco };
      });
      // Monaco retains initial mount callbacks; prop updates do not remount it.
      const mounted = useRef(false);
      useEffect(() => {
        if (mounted.current) return;
        mounted.current = true;
        mount.beforeMount?.(mount.monaco);
        mount.onMount?.(mount.editor, mount.monaco);
      }, [mount]);

      return (
        <textarea
          aria-label={options?.ariaLabel}
          data-read-only={options?.readOnly}
          data-testid="block-source-editor"
          readOnly={options?.readOnly}
          value={value}
          onChange={(event) => onChange?.(event.target.value)}
        />
      );
    }
  };
});

// Component-record editing does not exercise Monaco's browser worker setup.
vi.mock('../../../../shared/code-block/monaco-runtime', () => ({
  loadMonacoEditorModule: () => import('@monaco-editor/react')
}));

const uiManagementApi = vi.hoisted(() => ({
  settingsUiComponentsQueryKey: ['settings', 'ui-management', 'components'],
  settingsUiTemplatesQueryKey: ['settings', 'ui-management', 'templates'],
  fetchSettingsUiComponents: vi.fn(),
  fetchSettingsUiComponent: vi.fn(),
  createSettingsUiComponent: vi.fn(),
  updateSettingsUiComponent: vi.fn(),
  deleteSettingsUiComponent: vi.fn(),
  fetchSettingsUiCatalogPage: vi.fn(),
  searchSettingsUiCatalog: vi.fn(),
  downloadSettingsUiCatalogComponent: vi.fn(),
  fetchSettingsUiTemplates: vi.fn(),
  deleteSettingsUiTemplate: vi.fn(),
  createSettingsUiTemplate: vi.fn(),
  publishSettingsUiTemplate: vi.fn(),
  resetSettingsUiTemplateDefault: vi.fn(),
  setSettingsUiTemplateDefault: vi.fn(),
  updateSettingsUiTemplate: vi.fn()
}));

vi.mock('../../api/ui-management', () => uiManagementApi);

import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../shared/i18n/app-i18n';
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
  beforeAll(async () => {
    await loadApplicationI18nResources();
    await appI18n.changeLanguage('zh_Hans');
  });
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
    uiManagementApi.fetchSettingsUiCatalogPage.mockResolvedValue({
      catalog_version: '1.0.0',
      total_components: 1,
      page_size: 100,
      page: 1,
      cursor: 'start',
      next_cursor: null,
      records: [
        {
          ...official,
          catalog_updated_at: official.updated_at,
          source_locator: 'ui_components/@taichuy/ant-design-x/bubble.json',
          source_checksum: `sha256:${'a'.repeat(64)}`,
          local_version: official.version
        }
      ]
    });
    uiManagementApi.searchSettingsUiCatalog.mockResolvedValue({
      catalog_version: '1.0.0',
      page: 1,
      page_size: 20,
      total_entries: 1,
      entries: [
        { ...official, catalog_page: 1, local_version: official.version }
      ]
    });
    uiManagementApi.downloadSettingsUiCatalogComponent.mockResolvedValue(
      official
    );
  });

  test('shows load failure instead of an empty component list and recovers on retry', async () => {
    uiManagementApi.fetchSettingsUiComponents.mockRejectedValue(
      new Error('unavailable')
    );
    render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );
    expect(await screen.findByRole('alert')).toHaveTextContent(
      '组件加载失败，请重试。'
    );
    expect(screen.queryByText('暂无数据')).not.toBeInTheDocument();
    uiManagementApi.fetchSettingsUiComponents.mockResolvedValue([
      official,
      custom
    ]);
    fireEvent.click(screen.getByRole('button', { name: /重\s*试/ }));
    expect(await screen.findByText('Bubble')).toBeInTheDocument();
    expect(screen.getByText('Status panel')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  test('retains loaded components when refresh fails and clears the error on retry', async () => {
    render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );
    expect(await screen.findByText('Bubble')).toBeInTheDocument();
    uiManagementApi.fetchSettingsUiComponents.mockRejectedValue(
      new Error('unavailable')
    );
    fireEvent.click(screen.getByRole('button', { name: /刷\s*新/ }));
    expect(await screen.findByRole('alert')).toHaveTextContent(
      '组件加载失败，请重试。'
    );
    expect(screen.getByText('Bubble')).toBeInTheDocument();
    uiManagementApi.fetchSettingsUiComponents.mockResolvedValue([
      official,
      custom
    ]);
    fireEvent.click(screen.getByRole('button', { name: /重\s*试/ }));
    await waitFor(() =>
      expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    );
    expect(screen.getByText('Bubble')).toBeInTheDocument();
  });

  test('keeps a successful empty component list distinct from a request failure', async () => {
    uiManagementApi.fetchSettingsUiComponents.mockResolvedValue([]);
    render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );
    await waitFor(() =>
      expect(uiManagementApi.fetchSettingsUiComponents).toHaveBeenCalled()
    );
    expect(
      await screen.findByText('暂无数据', {
        selector: '.ant-empty-description'
      })
    ).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
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

  test('plugin-owned components are discoverable but cannot be edited or deleted', async () => {
    const plugin = {
      ...official,
      origin: 'plugin' as const,
      name: 'Calendar',
      component_code: 'plugin.calendar'
    };
    uiManagementApi.fetchSettingsUiComponents.mockResolvedValue([plugin]);
    uiManagementApi.fetchSettingsUiComponent.mockResolvedValue(plugin);
    render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );
    expect(await screen.findByText('Calendar')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '编辑 Calendar' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '删除 Calendar' })
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看 Calendar' }));
    const drawer = await screen.findByRole('dialog');
    expect(
      within(drawer).getByText('插件组件由已安装插件维护，不可在此修改。')
    ).toBeInTheDocument();
    expect(
      within(drawer).queryByRole('button', { name: /保存/ })
    ).not.toBeInTheDocument();
    expect(uiManagementApi.updateSettingsUiComponent).not.toHaveBeenCalled();
    expect(uiManagementApi.deleteSettingsUiComponent).not.toHaveBeenCalled();
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
    fireEvent.change(screen.getByLabelText('版本'), {
      target: { value: '0.1.0' }
    });
    fireEvent.change(screen.getByLabelText('记录版本'), {
      target: { value: '1.0.0' }
    });
    const editors = await screen.findAllByTestId('block-source-editor');
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

  test('WP-D3 browses the remote catalog as one table and updates a local record', async () => {
    render(
      <AppProviders>
        <UiManagementPanel canManage />
      </AppProviders>
    );
    await screen.findByText('Bubble');
    fireEvent.click(screen.getByRole('button', { name: '远程组件目录' }));
    const drawer = await screen.findByRole('dialog', {
      name: '远程组件目录'
    });
    expect(await within(drawer).findByText('Bubble')).toBeInTheDocument();
    expect(
      within(drawer).queryByText('taichuy / ant-design-x')
    ).not.toBeInTheDocument();
    expect(
      within(drawer).queryByRole('button', { name: '同步分组' })
    ).not.toBeInTheDocument();
    fireEvent.click(within(drawer).getByRole('button', { name: '更新' }));
    await waitFor(() =>
      expect(
        uiManagementApi.downloadSettingsUiCatalogComponent
      ).toHaveBeenCalledWith('taichuy.ant-design-x.bubble', 'csrf')
    );
  });
});
