import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const router = vi.hoisted(() => ({
  navigate: vi.fn(),
  pathname: '/settings/ui-management/code-templates'
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
const blockCatalogHook = vi.hoisted(() => ({
  useFrontstageBlockCatalog: vi.fn()
}));
const templateRunPanel = vi.hoisted(() => ({ render: vi.fn() }));
const sourceStudio = vi.hoisted(() => ({ render: vi.fn() }));

const templateCodeModules = [
  {
    source: '@1flowbase/block-sdk',
    version: '1.0.0',
    binding: 'fetched' as const,
    assets: [
      {
        role: 'browser_module' as const,
        media_type: 'text/javascript',
        sha256: 'a'.repeat(64),
        url: '/assets/block-sdk.js',
        integrity: 'verified_sha256' as const
      }
    ],
    exports: ['blockSdkVersion'],
    type_declarations: "declare module '@1flowbase/block-sdk' {}"
  },
  {
    source: '@1flowbase/native-components',
    version: '1.0.0',
    binding: 'fetched' as const,
    assets: [
      {
        role: 'browser_module' as const,
        media_type: 'text/javascript',
        sha256: 'b'.repeat(64),
        url: '/assets/native-components.js',
        integrity: 'verified_sha256' as const
      },
      {
        role: 'shadow_style' as const,
        media_type: 'text/css',
        sha256: 'c'.repeat(64),
        url: '/assets/native-components.css',
        integrity: 'verified_sha256' as const
      }
    ],
    exports: ['Surface'],
    type_declarations: "declare module '@1flowbase/native-components' {}"
  }
];
const previewSource = [
  "import type { BlockComponentProps } from '@1flowbase/block-sdk';",
  "import { Surface } from '@1flowbase/native-components';",
  'export default function Changed({ ctx }: BlockComponentProps) {',
  '  return <Surface>{ctx.workspace.id}</Surface>;',
  '}'
].join('\n');
const officialSource = previewSource.replace('Changed', 'OfficialBlock');
const managedSource = previewSource.replace('Changed', 'ManagedLatest');

vi.mock('../../api/ui-management', () => uiManagementApi);
vi.mock(
  '../../../frontstage/hooks/use-frontstage-block-catalog',
  () => blockCatalogHook
);
vi.mock(
  '../../../frontstage/components/jsx-studio/JsxStudioResourcePanel',
  () => ({
    JsxStudioResourcePanel: (props: {
      runPanel?: React.ReactNode;
      section: string;
    }) => (props.section === 'run' ? props.runPanel : null)
  })
);
vi.mock('../../../frontstage/components/jsx-studio/JsxStudioRunPanel', () => ({
  JsxStudioRunPanel: (props: unknown) => {
    templateRunPanel.render(props);
    return <div>Template preview run</div>;
  }
}));

vi.mock('../../../../shared/code-block/BlockSourceStudio', () => ({
  BlockSourceStudio: (props: {
    editorHeader?: React.ReactNode;
    editorDiagnostics?: Array<{ message: string }>;
    loading: boolean;
    onChange: (source: string) => void;
    onClose: () => void;
    onRun: (source: string) => void;
    onSave: () => void;
    readOnly: boolean;
    source: string;
    testId: string;
    renderResource: (section: 'configuration' | 'run') => React.ReactNode;
  }) => {
    sourceStudio.render(props);
    return (
      <section data-testid={props.testId}>
        <div data-testid="studio-editor-header">{props.editorHeader}</div>
        <aside data-testid="studio-resource-panel">
          {props.renderResource('configuration')}
        </aside>
        <span>{props.readOnly ? 'studio-readonly' : 'studio-editable'}</span>
        <pre data-testid="studio-source">{props.source}</pre>
        <button onClick={() => props.onChange(previewSource)}>
          change-source
        </button>
        <button
          disabled={props.loading}
          onClick={() => props.onRun(props.source)}
        >
          studio-run
        </button>
        <button onClick={props.onSave}>studio-save</button>
        <button onClick={props.onClose}>studio-close</button>
        <aside data-testid="studio-run-panel">
          {props.renderResource('run')}
        </aside>
      </section>
    );
  }
}));

import { AppProviders } from '../../../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { UiManagementPanel } from '../../components/ui-management/UiManagementPanel';

function renderPanel() {
  return render(
    <AppProviders>
      <UiManagementPanel canManage />
    </AppProviders>
  );
}

function studioSourceText(source: string) {
  return source.replace(/\s+/g, ' ');
}

describe('UiManagementPanel code templates', () => {
  beforeEach(() => {
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-token',
      actor: {
        id: 'user-1',
        user_id: 'user-1',
        current_workspace_id: 'workspace-1'
      } as never,
      me: null
    });
    uiManagementApi.fetchSettingsUiTemplates.mockResolvedValue({
      official: [
        {
          provider_code: '1flowbase',
          contribution_code: 'frontstage.js-ui-block',
          title: '官方区块',
          source: officialSource,
          language: 'tsx',
          version: '1.0.0',
          is_default: true
        }
      ],
      managed: [
        {
          id: 'managed-1',
          provider_code: '1flowbase',
          contribution_code: 'frontstage.js-ui-block',
          name: '自定义区块',
          latest_revision: {
            revision: 2,
            source: managedSource,
            language: 'tsx',
            is_published: false
          },
          published_revision: {
            revision: 1,
            source: 'export default function ManagedPublished() {}',
            language: 'tsx',
            is_published: true
          },
          is_default: false,
          is_archived: false
        }
      ]
    });
    uiManagementApi.createSettingsUiTemplate.mockResolvedValue({
      id: 'copy-1'
    });
    uiManagementApi.updateSettingsUiTemplate.mockResolvedValue({
      id: 'managed-1'
    });
    uiManagementApi.resetSettingsUiTemplateDefault.mockResolvedValue(undefined);
    uiManagementApi.setSettingsUiTemplateDefault.mockResolvedValue(undefined);
    blockCatalogHook.useFrontstageBlockCatalog.mockReturnValue({
      items: [
        {
          id: '1flowbase:frontstage.js-ui-block',
          runtimeKind: 'native_react',
          installationId: 'builtin-installation',
          providerCode: '1flowbase',
          pluginId: 'builtin-frontstage',
          pluginVersion: '1.0.0',
          contributionCode: 'frontstage.js-ui-block',
          entry: 'index.js',
          codeModules: templateCodeModules
        }
      ]
    });
  });

  afterEach(() => {
    resetAuthStore();
    vi.clearAllMocks();
  });

  test('AC-001 keeps the registered official template read-only while exposing view, copy, and default actions', async () => {
    renderPanel();

    expect(await screen.findByText('官方区块')).toBeInTheDocument();
    const officialRow = screen.getByText('官方区块').closest('tr');
    expect(officialRow).not.toBeNull();
    expect(officialRow).toHaveTextContent(/查\s*看/);
    expect(officialRow).toHaveTextContent(/复\s*制/);
    expect(officialRow).toHaveTextContent('设为默认');

    fireEvent.click(screen.getAllByRole('button', { name: /查\s*看/ })[0]!);
    expect(screen.getByTestId('ui-code-template-studio')).toHaveTextContent(
      'studio-readonly'
    );
    expect(screen.getByTestId('studio-source')).toHaveTextContent(
      studioSourceText(officialSource)
    );
  });

  test('AC-002 copies an official template snapshot into an independent managed draft', async () => {
    renderPanel();
    await screen.findByText('官方区块');

    fireEvent.click(screen.getAllByRole('button', { name: /复\s*制/ })[0]!);
    expect(screen.getByTestId('studio-source')).toHaveTextContent(
      studioSourceText(officialSource)
    );
    expect(screen.getByLabelText('名称')).toHaveValue('官方区块 - 副本');
    fireEvent.click(screen.getByRole('button', { name: 'change-source' }));
    fireEvent.click(screen.getByRole('button', { name: 'studio-save' }));

    await waitFor(() =>
      expect(uiManagementApi.createSettingsUiTemplate).toHaveBeenCalledWith(
        {
          provider_code: '1flowbase',
          contribution_code: 'frontstage.js-ui-block',
          name: '官方区块 - 副本',
          source: previewSource,
          language: 'tsx'
        },
        'csrf-token'
      )
    );
  });

  test('AC-003 copies the latest managed revision and edits managed templates in the same studio', async () => {
    renderPanel();
    await screen.findByText('自定义区块');

    fireEvent.click(screen.getAllByRole('button', { name: /复\s*制/ })[1]!);
    expect(screen.getByTestId('studio-source')).toHaveTextContent(
      studioSourceText(managedSource)
    );
    fireEvent.click(screen.getByRole('button', { name: 'studio-save' }));
    await waitFor(() =>
      expect(uiManagementApi.createSettingsUiTemplate).toHaveBeenCalledWith(
        expect.objectContaining({
          name: '自定义区块 - 副本',
          source: managedSource
        }),
        'csrf-token'
      )
    );

    fireEvent.click(screen.getByRole('button', { name: /编\s*辑/ }));
    expect(screen.getByTestId('studio-source')).toHaveTextContent(
      studioSourceText(managedSource)
    );
    expect(screen.getByTestId('ui-code-template-studio')).toHaveTextContent(
      'studio-editable'
    );
  });

  test('AC-004 starts new templates from a registered contribution instead of raw locator inputs', async () => {
    renderPanel();
    await screen.findByText('官方区块');

    fireEvent.click(screen.getByRole('button', { name: '新建模板' }));

    expect(screen.getByTestId('ui-code-template-studio')).toBeInTheDocument();
    expect(screen.getByTestId('studio-editor-header')).toBeEmptyDOMElement();
    expect(screen.getByTestId('studio-resource-panel')).toContainElement(
      screen.getByLabelText('所属区块')
    );
    expect(screen.getByLabelText('所属区块')).toBeInTheDocument();
    expect(screen.queryByLabelText('提供方代码')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('贡献代码')).not.toBeInTheDocument();

    fireEvent.mouseDown(screen.getByLabelText('所属区块'));
    fireEvent.click(
      await screen.findByText('官方区块 · 1flowbase/frontstage.js-ui-block')
    );
    fireEvent.change(screen.getByLabelText('名称'), {
      target: { value: '新模板' }
    });
    fireEvent.click(screen.getByRole('button', { name: 'studio-save' }));
    await waitFor(() =>
      expect(uiManagementApi.createSettingsUiTemplate).toHaveBeenCalledWith(
        {
          provider_code: '1flowbase',
          contribution_code: 'frontstage.js-ui-block',
          name: '新模板',
          source: officialSource,
          language: 'tsx'
        },
        'csrf-token'
      )
    );
  });

  test('AC-006 runs an unsaved template draft through the registered block runtime', async () => {
    renderPanel();
    await screen.findByText('自定义区块');

    fireEvent.click(screen.getByRole('button', { name: /编\s*辑/ }));
    fireEvent.click(screen.getByRole('button', { name: 'change-source' }));
    const runButton = screen.getByRole('button', { name: 'studio-run' });
    await waitFor(() => expect(runButton).toBeEnabled());
    fireEvent.click(runButton);

    expect(await screen.findByText('Template preview run')).toBeInTheDocument();
    expect(
      screen.queryByText(
        '当前为受限预览：可验证组件渲染与本地交互，接口调用、事件、导航和输出能力不可用。'
      )
    ).not.toBeInTheDocument();
    const runProps = templateRunPanel.render.mock.calls.at(-1)?.[0] as {
      block: { id: string; props: Record<string, unknown> };
      code: string;
      revision: string;
      createBlockContext(input: {
        plan: {
          blockId: string;
          props: Record<string, unknown>;
        };
      }): {
        currentUser: { id: string; displayName?: string } | null;
        workspace: { id: string };
        page: { id: string; route: string; title?: string };
        ui: { locale?: string };
        props: Record<string, unknown>;
      };
    };
    expect(runProps.block).toMatchObject({
      id: 'ui-code-template:1flowbase:frontstage.js-ui-block',
      props: {}
    });
    expect(runProps.code).toBe(previewSource);
    expect(runProps.revision).toBe('run:1');
    const studioProps = sourceStudio.render.mock.calls.at(-1)?.[0] as {
      editorDiagnostics: Array<{ message: string }>;
    };
    expect(studioProps.editorDiagnostics).toEqual([]);

    const previewContext = runProps.createBlockContext({
      plan: {
        blockId: runProps.block.id,
        props: {}
      }
    });
    expect(previewContext.currentUser).toEqual(
      expect.objectContaining({ id: 'user-1' })
    );
    expect(previewContext.workspace).toEqual({ id: 'workspace-1' });
    expect(previewContext.page).toEqual({
      id: runProps.block.id,
      route:
        '/settings/ui-management/code-templates/1flowbase%3Afrontstage.js-ui-block',
      title: '自定义区块'
    });
    expect(previewContext.ui).toEqual({ locale: undefined });
    expect(previewContext.props).toEqual({});
    expect(uiManagementApi.updateSettingsUiTemplate).not.toHaveBeenCalled();
  });

  test('AC-007 waits for the contribution catalog before validating registered imports', async () => {
    blockCatalogHook.useFrontstageBlockCatalog.mockReturnValue({
      items: [],
      loading: true
    });
    renderPanel();
    await screen.findByText('自定义区块');

    fireEvent.click(screen.getByRole('button', { name: /编\s*辑/ }));

    const studioProps = sourceStudio.render.mock.calls.at(-1)?.[0] as {
      editorDiagnostics: Array<{ message: string }>;
      loading: boolean;
    };
    expect(studioProps.loading).toBe(true);
    expect(studioProps.editorDiagnostics).toEqual([]);
    expect(screen.getByRole('button', { name: 'studio-run' })).toBeDisabled();
  });

  test('AC-005 switches the configurable default between managed and official templates', async () => {
    const view = renderPanel();
    await screen.findByText('官方区块');

    const defaultButtons = screen.getAllByRole('button', { name: '设为默认' });
    expect(defaultButtons[0]).toBeDisabled();
    fireEvent.click(defaultButtons[1]!);
    await waitFor(() =>
      expect(uiManagementApi.setSettingsUiTemplateDefault).toHaveBeenCalledWith(
        'managed-1',
        'csrf-token'
      )
    );

    view.unmount();
    uiManagementApi.fetchSettingsUiTemplates.mockResolvedValue({
      official: [
        {
          provider_code: '1flowbase',
          contribution_code: 'frontstage.js-ui-block',
          title: '官方区块',
          source: officialSource,
          language: 'tsx',
          version: '1.0.0',
          is_default: false
        }
      ],
      managed: [
        {
          id: 'managed-1',
          provider_code: '1flowbase',
          contribution_code: 'frontstage.js-ui-block',
          name: '自定义区块',
          latest_revision: {
            revision: 2,
            source: managedSource,
            language: 'tsx',
            is_published: true
          },
          published_revision: {
            revision: 2,
            source: managedSource,
            language: 'tsx',
            is_published: true
          },
          is_default: true,
          is_archived: false
        }
      ]
    });
    renderPanel();
    await screen.findByText('官方区块');
    fireEvent.click(screen.getAllByRole('button', { name: '设为默认' })[0]!);
    await waitFor(() =>
      expect(
        uiManagementApi.resetSettingsUiTemplateDefault
      ).toHaveBeenCalledWith(
        {
          provider_code: '1flowbase',
          contribution_code: 'frontstage.js-ui-block'
        },
        'csrf-token'
      )
    );
  });
});
