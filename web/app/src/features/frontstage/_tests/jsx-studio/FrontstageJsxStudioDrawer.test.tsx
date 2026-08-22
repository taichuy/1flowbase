import { setImmediate } from 'node:timers/promises';

import {
  // eslint-disable-next-line testing-library/no-manual-cleanup -- Explicit teardown drains React scheduler work before jsdom removes window.
  cleanup,
  fireEvent,
  render as testingRender,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { App } from 'antd';
import type { ReactElement, ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import type { ConsoleFrontstageComponentCapabilitySummary } from '@1flowbase/api-client';
import {
  LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC,
  sha256Text
} from '@1flowbase/page-runtime';

import { appI18n } from '../../../../shared/i18n/app-i18n';
import { FrontstageJsxStudioDrawer } from '../../components/jsx-studio/FrontstageJsxStudioDrawer';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import type { FrontstageBlockInstance } from '../../lib/page-document';

const render = (ui: ReactElement) => testingRender(ui, { wrapper: App });

const blockCodeHook = vi.hoisted(() => ({
  useFrontstageBlockCode: vi.fn()
}));
const blockTabsHook = vi.hoisted(() => ({
  useFrontstageBlockTabs: vi.fn()
}));
const interfaceCapabilitiesHook = vi.hoisted(() => ({
  useFrontstageInterfaceCapabilities: vi.fn(),
  useFrontstageInterfaceCapabilityDetails: vi.fn()
}));
const interfaceCapabilitiesApi = vi.hoisted(() => ({
  fetchFrontstageInterfaceCapability: vi.fn()
}));
const componentCapabilitiesHook = vi.hoisted(() => ({
  useFrontstageComponentCapabilities: vi.fn()
}));
const componentCapabilitiesApi = vi.hoisted(() => ({
  fetchFrontstageComponentCapability: vi.fn()
}));
const uiTemplatesHook = vi.hoisted(() => ({
  useFrontstageUiTemplates: vi.fn()
}));
const antdAppMocks = vi.hoisted(() => ({
  error: vi.fn(),
  success: vi.fn(),
  warning: vi.fn()
}));
const monacoHook = vi.hoisted(() => ({
  addExtraLib: vi.fn(),
  setCompilerOptions: vi.fn(),
  setModelMarkers: vi.fn()
}));
const monacoEditor = vi.hoisted(() => ({
  executeEdits: vi.fn(),
  focus: vi.fn(),
  getModel: vi.fn(),
  getSelection: vi.fn(),
  pushUndoStop: vi.fn()
}));
const pageRuntimeMocks = vi.hoisted(() => ({
  createJsBlockDiagnostics: vi.fn()
}));

vi.mock('../../hooks/use-frontstage-block-code', () => blockCodeHook);
vi.mock('@1flowbase/page-runtime', async () => {
  const actual = await vi.importActual<
    typeof import('@1flowbase/page-runtime')
  >('@1flowbase/page-runtime');
  return {
    ...actual,
    createJsBlockDiagnostics: (
      ...args: Parameters<typeof actual.createJsBlockDiagnostics>
    ) => {
      pageRuntimeMocks.createJsBlockDiagnostics(...args);
      return actual.createJsBlockDiagnostics(...args);
    }
  };
});
vi.mock(
  '../../components/jsx-studio/block-tabs/use-frontstage-block-tabs',
  () => blockTabsHook
);
vi.mock(
  '../../hooks/use-frontstage-interface-capabilities',
  () => interfaceCapabilitiesHook
);
vi.mock('../../api/interface-capabilities', () => interfaceCapabilitiesApi);
vi.mock(
  '../../hooks/use-frontstage-component-capabilities',
  () => componentCapabilitiesHook
);
vi.mock('../../api/component-capabilities', () => componentCapabilitiesApi);
vi.mock('../../hooks/use-frontstage-ui-templates', () => uiTemplatesHook);
vi.mock('antd', async () => {
  const actual = await vi.importActual<typeof import('antd')>('antd');
  return {
    ...actual,
    App: Object.assign(actual.App, {
      useApp: () => ({ message: antdAppMocks })
    })
  };
});
vi.mock('../../../../shared/ui/resizable-drawer/ResizableDrawer', () => ({
  ResizableDrawer: ({
    children,
    extra,
    title
  }: {
    children: ReactNode;
    extra?: ReactNode;
    title: ReactNode;
  }) => (
    <dialog aria-label={String(title)} open>
      {extra}
      {children}
    </dialog>
  )
}));
vi.mock('@monaco-editor/react', () => ({
  default: ({
    beforeMount,
    value,
    onChange,
    onMount,
    options
  }: {
    beforeMount?: (monaco: unknown) => void;
    value?: string;
    onChange?: (value?: string) => void;
    onMount?: (editor: unknown, monaco: unknown) => void;
    options?: { editContext?: boolean };
  }) => {
    const monaco = {
      MarkerSeverity: { Error: 8 },
      editor: { setModelMarkers: monacoHook.setModelMarkers },
      languages: {
        typescript: {
          JsxEmit: { Preserve: 'preserve', ReactJSX: 'react-jsx' },
          ModuleResolutionKind: { NodeJs: 'node-js' },
          ScriptTarget: { ES2022: 'es2022' },
          typescriptDefaults: {
            addExtraLib: monacoHook.addExtraLib,
            setCompilerOptions: monacoHook.setCompilerOptions
          }
        }
      }
    };
    beforeMount?.(monaco);
    onMount?.(monacoEditor, monaco);
    return (
      <textarea
        aria-label="JSX source"
        data-edit-context={String(options?.editContext)}
        value={value}
        onChange={(event) => onChange?.(event.target.value)}
      />
    );
  }
}));

const block: FrontstageBlockInstance = {
  id: 'orders-block',
  rendererVersion: 'v1',
  sourceId: 'orders-block',
  codeRef: 'orders-code',
  sourceCodeRef: 'orders-code',
  catalog: {
    providerCode: '1flowbase',
    installationId: 'builtin-installation'
  },
  contribution: {
    pluginId: 'builtin-frontstage',
    pluginVersion: '1.0.0',
    code: 'frontstage.js-ui-block'
  },
  props: { title: 'Orders' },
  presentation: { heightMode: 'auto', height: null },
  layout: { order: 0 },
  order: 0,
  runtime: { kind: 'native_react', entry: 'index.js', hint: 'native_react' }
};

const catalogEntry: NormalizedFrontstageBlockCatalogEntry = {
  id: '1flowbase:frontstage.js-ui-block',
  runtimeKind: 'native_react',
  installationId: 'builtin-installation',
  providerCode: '1flowbase',
  pluginId: 'builtin-frontstage',
  pluginVersion: '1.0.0',
  contributionCode: 'frontstage.js-ui-block',
  title: 'JSX 区块',
  entry: 'index.js',
  permissions: { network: 'none', storage: 'none', secrets: 'none' },
  contextContract: { primitives: [], inputSchema: {} },
  uiCapabilities: ['configurable', 'data_binding'],
  codeModules: [
    {
      source: '@1flowbase/native-components',
      version: '1.0.0',
      binding: 'fetched',
      assets: [
        {
          role: 'browser_module',
          media_type: 'text/javascript; charset=utf-8',
          sha256:
            'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
          url: '/fixture-assets/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
        }
      ],
      exports: ['Button'],
      type_declarations:
        "declare module '@1flowbase/native-components' { export const Button: unknown; }"
    }
  ],
  codeCapabilities: {
    template: null,
    allowedImports: ['@1flowbase/native-components'],
    monacoExtraLibs: [
      {
        source: '@1flowbase/native-components',
        filePath:
          'file:///node_modules/@1flowbase/native-components/index.d.ts',
        content:
          "declare module '@1flowbase/native-components' { export const Button: unknown; }"
      }
    ]
  },
  raw: {} as NormalizedFrontstageBlockCatalogEntry['raw']
};

const buttonComponent = {
  component_id: 'builtin-installation:frontstage.js-ui-block:button',
  installation_id: 'builtin-installation',
  provider_code: '1flowbase',
  plugin_id: 'builtin-frontstage',
  plugin_version: '1.0.0',
  contribution_code: 'frontstage.js-ui-block',
  module_source: '@1flowbase/native-components',
  module_version: '1.0.0',
  browser_asset: {
    sha256: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
    url: '/api/console/frontstage/component-module-assets/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
  },
  export_name: 'Button',
  upstream: null,
  description: 'Native button component.',
  insert_snippet: '<Button>Action</Button>'
} satisfies ConsoleFrontstageComponentCapabilitySummary;

describe('FrontstageJsxStudioDrawer', () => {
  afterEach(async () => {
    cleanup();
    // React's scheduler may enqueue a follow-up Immediate while processing the
    // first one. Drain both turns before Vitest removes the jsdom window.
    await setImmediate();
    await setImmediate();
  });

  beforeEach(async () => {
    vi.clearAllMocks();
    monacoHook.addExtraLib.mockReturnValue({ dispose: vi.fn() });
    monacoEditor.getSelection.mockReturnValue(null);
    monacoEditor.getModel.mockReturnValue(null);
    Object.defineProperty(window, 'innerWidth', {
      configurable: true,
      value: 1400
    });
    Object.defineProperty(window, 'innerHeight', {
      configurable: true,
      value: 900
    });
    await appI18n.changeLanguage('zh_Hans');
    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: 'export default {}',
      draft: 'export default {}',
      dirty: false,
      loading: false,
      saving: false,
      error: null,
      permissionDenied: false,
      setDraft: vi.fn(),
      reset: vi.fn(),
      save: vi.fn().mockResolvedValue(undefined)
    });
    blockTabsHook.useFrontstageBlockTabs.mockImplementation(
      ({ initialBlockId }: { initialBlockId: string }) => {
        const legacy = blockCodeHook.useFrontstageBlockCode();
        const tab = {
          block_id: initialBlockId,
          detail: {
            block_id: initialBlockId,
            tab_id: 'tab-1',
            title: 'Orders'
          },
          base_source: legacy.code,
          draft: legacy.draft,
          source_sha256: sha256Text(legacy.code),
          executable: {
            block_id: initialBlockId,
            page_id: 'page-1',
            source_code: legacy.code,
            source_sha256: sha256Text(legacy.code),
            dependency_lock: []
          },
          loading: legacy.loading,
          saving: legacy.saving,
          error: legacy.error
        };
        return {
          tabs: [tab],
          activeBlockId: initialBlockId,
          activeTab: tab,
          anyDirty: legacy.dirty,
          openBlock: vi.fn(),
          activateBlock: vi.fn(),
          closeBlock: vi.fn(),
          setDraft: vi.fn((_blockId: string, draft: string) =>
            legacy.setDraft(draft)
          ),
          setActiveDraft: legacy.setDraft,
          resetActive: legacy.reset,
          saveActiveDraft: legacy.save,
          handleDeletedBlock: vi.fn().mockResolvedValue('converged')
        };
      }
    );
    uiTemplatesHook.useFrontstageUiTemplates.mockReturnValue({
      data: [],
      isLoading: false
    });
    const capability = {
      interface_id: 'list_application_conversations_records',
      method: 'GET',
      path: '/api/runtime/models/application_conversations/list',
      name: 'List conversations',
      short_description: 'List conversations',
      parameter_schema: { type: 'object', properties: {} },
      result_schema: { type: 'object', properties: {} },
      request_media_type: null,
      response_media_type: 'application/json',
      schema_digest: 'digest-1',
      adapter_id: 'runtime_data_model',
      host_injected_parameters: [],
      scope: 'frontstage_page_tab',
      risk_level: 'low',
      authorization: 'runtime_scope_grant_and_page_tab_access',
      bindable: true,
      disabled_reason: null
    };
    interfaceCapabilitiesHook.useFrontstageInterfaceCapabilityDetails.mockReturnValue(
      {
        data: [],
        loading: false,
        error: null
      }
    );
    interfaceCapabilitiesHook.useFrontstageInterfaceCapabilities.mockReturnValue(
      {
        data: {
          items: [
            {
              interface_id: capability.interface_id,
              method: capability.method,
              path: capability.path,
              adapter_id: capability.adapter_id
            }
          ],
          total: 1,
          offset: 0,
          limit: 10,
          has_more: false,
          next_offset: null,
          adapter_ids: ['runtime_data_model'],
          methods: ['GET']
        },
        loading: false,
        error: null
      }
    );
    interfaceCapabilitiesApi.fetchFrontstageInterfaceCapability.mockResolvedValue(
      capability
    );
    componentCapabilitiesHook.useFrontstageComponentCapabilities.mockReturnValue(
      {
        data: {
          items: [buttonComponent],
          total: 1,
          offset: 0,
          limit: 10,
          has_more: false,
          next_offset: null,
          module_sources: [buttonComponent.module_source]
        },
        loading: false,
        error: null
      }
    );
    componentCapabilitiesApi.fetchFrontstageComponentCapability.mockResolvedValue(
      {
        ...buttonComponent,
        props: [],
        limitations: [],
        examples: [],
        typescript_declaration:
          "declare module '@1flowbase/native-components' { export const Button: unknown; }",
        api_documentation: ''
      }
    );
  });

  test('accepts full Tailwind and custom classes without private inventory diagnostics', () => {
    const activeSource =
      'import \'tailwindcss\'; export default function ActiveBlock() { return <div className="grid grid-cols-[200px_1fr] bg-[#00ab73] md:grid-cols-2 custom-layout" />; }';
    const tailwindCatalogEntry: NormalizedFrontstageBlockCatalogEntry = {
      ...catalogEntry,
      codeModules: [
        ...(catalogEntry.codeModules ?? []),
        {
          source: 'tailwindcss',
          version: '4.3.3',
          binding: 'fetched',
          assets: [],
          exports: ['default'],
          type_declarations:
            'declare module "tailwindcss" { const value: unknown; export default value; }'
        }
      ]
    };
    const activeTab = {
      block_id: 'active-block',
      detail: {
        block_id: 'active-block',
        tab_id: 'active-tab',
        title: 'Active block'
      },
      base_source: activeSource,
      draft: activeSource,
      source_sha256: 'active-sha256',
      loading: false,
      saving: false,
      error: null
    };
    blockTabsHook.useFrontstageBlockTabs.mockReturnValue({
      tabs: [activeTab],
      activeBlockId: activeTab.block_id,
      activeTab,
      anyDirty: false,
      openBlock: vi.fn(),
      activateBlock: vi.fn(),
      closeBlock: vi.fn(),
      setActiveDraft: vi.fn(),
      resetActive: vi.fn(),
      saveActiveDraft: vi.fn().mockResolvedValue(undefined),
      handleDeletedBlock: vi.fn().mockResolvedValue('converged')
    });

    render(
      <FrontstageJsxStudioDrawer
        open
        initialSection="code"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="initial-tab"
        block={block}
        catalogEntry={tailwindCatalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );

    expect(pageRuntimeMocks.createJsBlockDiagnostics).toHaveBeenCalledWith(
      {
        pageId: 'page-1',
        tabId: 'active-tab',
        blockId: 'active-block'
      },
      []
    );
  });

  test('AC-004/009 exposes block tree without the legacy child-container write rail', () => {
    render(
      <FrontstageJsxStudioDrawer
        open
        initialSection="code"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        pageBlocks={[block]}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );

    expect(
      screen.queryByRole('button', { name: '子容器' })
    ).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '区块树' })).toBeInTheDocument();
  });

  test('keeps Monaco visible while configuration and interface resources share one Studio', async () => {
    const onSaveBlock = vi.fn().mockResolvedValue(true);

    render(
      <FrontstageJsxStudioDrawer
        open
        initialSection="configuration"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={onSaveBlock}
      />
    );

    expect(screen.getByRole('dialog', { name: 'TSX 编辑器' })).toHaveStyle({
      height: '680px'
    });
    expect(screen.getByLabelText('JSX source')).toBeInTheDocument();
    expect(screen.getByLabelText('JSX source')).toHaveAttribute(
      'data-edit-context',
      'false'
    );
    expect(
      screen
        .getByLabelText('标题')
        .closest('.frontstage-jsx-studio__configuration-panel')
    ).not.toBeNull();
    expect(
      interfaceCapabilitiesHook.useFrontstageInterfaceCapabilities
    ).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: '接口' }));
    fireEvent.click(
      await screen.findByText(
        '/api/runtime/models/application_conversations/list'
      )
    );
    fireEvent.click(screen.getByRole('button', { name: '插入代码' }));

    await waitFor(() =>
      expect(antdAppMocks.success).toHaveBeenCalledWith('接口代码已插入')
    );
    expect(onSaveBlock).not.toHaveBeenCalled();
  });

  test('AC-001 attributes a Monaco change to the block identified by its model path', () => {
    const setDraft = vi.fn();
    const rootTab = {
      block_id: 'root',
      detail: { block_id: 'root', tab_id: 'tab-1', title: 'Root' },
      base_source: 'root source',
      draft: 'root source',
      source_sha256: 'root-sha256',
      loading: false,
      saving: false,
      error: null
    };
    const childTab = {
      ...rootTab,
      block_id: 'child',
      detail: { block_id: 'child', tab_id: 'tab-1', title: 'Child' },
      base_source: '',
      draft: ''
    };
    blockTabsHook.useFrontstageBlockTabs.mockReturnValue({
      tabs: [rootTab, childTab],
      activeBlockId: 'child',
      activeTab: childTab,
      anyDirty: false,
      openBlock: vi.fn(),
      activateBlock: vi.fn(),
      closeBlock: vi.fn(),
      setDraft,
      setActiveDraft: vi.fn(),
      resetActive: vi.fn(),
      saveActiveDraft: vi.fn().mockResolvedValue(undefined),
      handleDeletedBlock: vi.fn().mockResolvedValue('converged')
    });
    monacoEditor.getModel.mockReturnValue({
      uri: {
        toString: () => 'file:///frontstage/page-1/blocks/child.tsx'
      }
    });

    render(
      <FrontstageJsxStudioDrawer
        open
        initialSection="code"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );
    fireEvent.change(screen.getByRole('textbox', { name: 'JSX source' }), {
      target: { value: 'child draft' }
    });

    expect(setDraft).toHaveBeenCalledWith('child', 'child draft');
  });

  test('AC-001/002/003 keeps templates in their own resource section and replaces the whole draft', async () => {
    const setDraft = vi.fn();
    const save = vi.fn().mockResolvedValue(undefined);
    const templateSource = `export default function TemplateBlock() {
  return <section>Template</section>;
}`;
    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: 'export default function CurrentBlock() { return null; }',
      draft: 'export default function CurrentBlock() { return null; }',
      dirty: false,
      loading: false,
      saving: false,
      error: null,
      permissionDenied: false,
      setDraft,
      reset: vi.fn(),
      save
    });
    uiTemplatesHook.useFrontstageUiTemplates.mockReturnValue({
      data: [
        {
          template_id: 'dashboard-template',
          provider_code: '1flowbase',
          contribution_code: 'frontstage.js-ui-block',
          name: 'Dashboard',
          source: templateSource,
          language: 'tsx',
          version: '2.1.0',
          is_official: false,
          is_default: true
        },
        {
          template_id: 'other-template',
          provider_code: 'other-provider',
          contribution_code: 'other-block',
          name: 'Other template',
          source: 'export default {}',
          language: 'tsx',
          version: '1.0.0',
          is_official: false,
          is_default: false
        }
      ],
      isLoading: false
    });

    const view = render(
      <FrontstageJsxStudioDrawer
        open
        initialSection="code"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );

    const editorPanel = view.container.querySelector(
      '.frontstage-jsx-studio__editor-panel'
    );
    expect(
      within(editorPanel as HTMLElement).queryByText('代码模板')
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '代码模板' }));
    expect(
      screen.getByRole('heading', { name: '代码模板' })
    ).toBeInTheDocument();
    expect(screen.getByText('Dashboard · 2.1.0 · 默认')).toBeInTheDocument();
    expect(screen.queryByText('Other template')).not.toBeInTheDocument();

    fireEvent.click(screen.getByText('Dashboard · 2.1.0 · 默认'));
    fireEvent.click(screen.getByRole('button', { name: '替换当前代码' }));
    expect(await screen.findAllByText('替换当前代码？')).not.toHaveLength(0);
    fireEvent.click(
      within(screen.getByRole('dialog', { name: '替换当前代码？' })).getByRole(
        'button',
        { name: /^替\s*换$/ }
      )
    );

    await waitFor(() => expect(setDraft).toHaveBeenCalledWith(templateSource));
    expect(save).not.toHaveBeenCalled();
  });

  test('resizes the resource panel horizontally without resizing the Studio window', () => {
    render(
      <FrontstageJsxStudioDrawer
        open
        initialSection="interfaces"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );

    const dialog = screen.getByRole('dialog', { name: 'TSX 编辑器' });
    const workspace = dialog.querySelector<HTMLElement>(
      '.frontstage-jsx-studio__workspace'
    );
    const resizeHandle = screen.getByRole('separator', {
      name: '调整资源面板宽度'
    });

    expect(workspace?.style.getPropertyValue('--resource-panel-width')).toBe(
      '320px'
    );
    fireEvent.mouseDown(resizeHandle, { clientX: 500 });
    fireEvent.mouseMove(document, { clientX: 400 });
    fireEvent.mouseUp(document);
    expect(workspace?.style.getPropertyValue('--resource-panel-width')).toBe(
      '420px'
    );
    expect(dialog).toHaveStyle({ width: '1080px' });

    fireEvent.keyDown(resizeHandle, { key: 'ArrowRight' });
    expect(workspace?.style.getPropertyValue('--resource-panel-width')).toBe(
      '380px'
    );
  });

  test('preserves JSX in Monaco because Page Runtime owns TSX compilation', () => {
    render(
      <FrontstageJsxStudioDrawer
        open
        initialSection="code"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );

    expect(monacoHook.setCompilerOptions).toHaveBeenCalledWith(
      expect.objectContaining({ jsx: 'preserve' })
    );
  });

  test('R5-AC-001/002 uses Catalog imports for inline Monaco problems', async () => {
    const model = {};
    monacoEditor.getModel.mockReturnValue(model);
    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: "import { Button } from '@1flowbase/native-components';",
      draft: "import { Button } from '@1flowbase/native-components';",
      dirty: false,
      loading: false,
      saving: false,
      error: null,
      permissionDenied: false,
      setDraft: vi.fn(),
      reset: vi.fn(),
      save: vi.fn().mockResolvedValue(undefined)
    });
    const props = {
      open: true,
      initialSection: 'code' as const,
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      block,
      catalogEntry,
      onClose: vi.fn(),
      onSaveBlock: vi.fn()
    };
    const view = render(<FrontstageJsxStudioDrawer {...props} />);

    expect(monacoHook.setModelMarkers).toHaveBeenLastCalledWith(
      model,
      expect.any(String),
      []
    );
    expect(screen.queryByText('代码诊断')).not.toBeInTheDocument();

    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: "import dayjs from 'dayjs';",
      draft: "import dayjs from 'dayjs';",
      dirty: false,
      loading: false,
      saving: false,
      error: null,
      permissionDenied: false,
      setDraft: vi.fn(),
      reset: vi.fn(),
      save: vi.fn().mockResolvedValue(undefined)
    });
    view.rerender(<FrontstageJsxStudioDrawer {...props} />);

    await waitFor(() =>
      expect(monacoHook.setModelMarkers).toHaveBeenLastCalledWith(
        model,
        expect.any(String),
        [
          expect.objectContaining({
            code: 'import_denied',
            message: "Import source 'dayjs' is not allowed.",
            startLineNumber: 1,
            startColumn: 1
          })
        ]
      )
    );
    expect(screen.queryByText('代码诊断')).not.toBeInTheDocument();
  });

  test('AC-005 submits snippet and import changes as one Monaco edit batch', async () => {
    const source = `export default function Block({ ctx }: NativeReactBlockProps) {
  return <div>content</div>;
}`;
    const selectionOffset = source.indexOf('content');
    const positionAt = (offset: number) => {
      const before = source.slice(0, offset).split('\n');
      return {
        lineNumber: before.length,
        column: (before.at(-1)?.length ?? 0) + 1
      };
    };
    const offsetAt = ({
      lineNumber,
      column
    }: {
      lineNumber: number;
      column: number;
    }) => {
      const lines = source.split('\n');
      return (
        lines
          .slice(0, lineNumber - 1)
          .reduce((total, line) => total + line.length + 1, 0) +
        column -
        1
      );
    };
    const selectionPosition = positionAt(selectionOffset);
    monacoEditor.getSelection.mockReturnValue({
      getStartPosition: () => selectionPosition,
      getEndPosition: () => selectionPosition
    });
    monacoEditor.getModel.mockReturnValue({
      getValue: () => source,
      getOffsetAt: offsetAt,
      getPositionAt: positionAt
    });
    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: source,
      draft: source,
      dirty: false,
      loading: false,
      saving: false,
      error: null,
      permissionDenied: false,
      setDraft: vi.fn(),
      reset: vi.fn(),
      save: vi.fn().mockResolvedValue(undefined)
    });

    render(
      <FrontstageJsxStudioDrawer
        open
        initialSection="components"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );
    const row = screen.getByText('Button').closest('tr');
    fireEvent.click(within(row!).getByRole('button', { name: '插入' }));

    await waitFor(() =>
      expect(monacoEditor.executeEdits).toHaveBeenCalledTimes(1)
    );
    expect(monacoEditor.executeEdits.mock.calls[0]?.[1]).toHaveLength(2);
    expect(monacoEditor.pushUndoStop).toHaveBeenCalledTimes(2);
  });

  test('AC-004 saves fixed block height from the configuration section', async () => {
    const onSaveBlock = vi.fn().mockResolvedValue(true);

    render(
      <FrontstageJsxStudioDrawer
        open
        initialSection="configuration"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={onSaveBlock}
      />
    );

    fireEvent.mouseDown(screen.getByRole('combobox', { name: '高度模式' }));
    fireEvent.click(await screen.findByText('固定高度'));
    fireEvent.change(screen.getByRole('spinbutton', { name: '固定高度' }), {
      target: { value: '360' }
    });
    fireEvent.click(screen.getByRole('button', { name: '保存配置' }));

    await waitFor(() => expect(onSaveBlock).toHaveBeenCalledTimes(1));
    expect(onSaveBlock.mock.calls[0]?.[0]).toMatchObject({
      presentation: { heightMode: 'fixed', height: 360 }
    });
  });

  test('AC-003 does not save the host block title into runtime props', async () => {
    const onSaveBlock = vi.fn().mockResolvedValue(true);
    const onSaveBlockTitle = vi.fn().mockResolvedValue(true);
    render(
      <FrontstageJsxStudioDrawer
        open
        initialSection="configuration"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={{ ...block, props: {} }}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={onSaveBlock}
        onSaveBlockTitle={onSaveBlockTitle}
      />
    );

    fireEvent.change(screen.getByRole('textbox', { name: '标题' }), {
      target: { value: 'K7M2PX9Q' }
    });
    fireEvent.click(screen.getByRole('button', { name: '保存配置' }));

    await waitFor(() => expect(onSaveBlock).toHaveBeenCalledTimes(1));
    expect(onSaveBlock.mock.calls[0]?.[0].props).not.toHaveProperty('title');
    expect(onSaveBlockTitle).toHaveBeenCalledWith('orders-block', 'K7M2PX9Q');
  });

  test('AC-006 declares a typed output port from the variables section', async () => {
    const onSaveBlock = vi.fn().mockResolvedValue(true);
    render(
      <FrontstageJsxStudioDrawer
        open
        initialSection="variables"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        pageBlocks={[block]}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={onSaveBlock}
      />
    );
    fireEvent.change(screen.getByRole('textbox', { name: '输出端口' }), {
      target: { value: 'total' }
    });
    fireEvent.click(screen.getByRole('button', { name: '添加端口' }));
    await waitFor(() => expect(onSaveBlock).toHaveBeenCalledTimes(1));
    expect(onSaveBlock.mock.calls[0]?.[0]).toMatchObject({
      ports: { outputs: [{ name: 'total', schema: { type: 'string' } }] }
    });
  });

  test('AC-043/044/046 runs from the header and keeps save persistence separate', async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const setDraft = vi.fn();
    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: 'export default {}',
      draft: 'export default {}',
      dirty: true,
      loading: false,
      saving: false,
      error: null,
      permissionDenied: false,
      setDraft,
      reset: vi.fn(),
      save
    });

    render(
      <FrontstageJsxStudioDrawer
        open
        initialSection="code"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        runPanel={({ code, runRevision }) => (
          <div>{`Preview ${runRevision ?? 'idle'}: ${code}`}</div>
        )}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );

    const windowHeader = document.querySelector(
      '.frontstage-jsx-studio__window-header'
    );
    expect(windowHeader).not.toBeNull();
    expect(
      windowHeader?.querySelector('.frontstage-jsx-studio__window-actions')
    ).toHaveStyle({ flexWrap: 'wrap' });
    const headerButtons = within(windowHeader as HTMLElement).getAllByRole(
      'button'
    );
    expect(
      headerButtons
        .slice(0, 4)
        .map((button) => button.textContent?.replace(/\s+/gu, ''))
    ).toEqual(['上下文', '重置', '保存', '运行']);
    expect(
      within(windowHeader as HTMLElement).getByRole('button', {
        name: /^运\s*行$/
      })
    ).toHaveClass('ant-btn-primary');
    expect(
      within(windowHeader as HTMLElement).getByRole('button', {
        name: /保\s*存/
      })
    ).not.toHaveClass('ant-btn-primary');
    fireEvent.click(
      within(windowHeader as HTMLElement).getByRole('button', {
        name: '上下文'
      })
    );
    expect(setDraft).toHaveBeenCalledWith(
      expect.stringContaining('@1flowbase-context')
    );
    fireEvent.click(screen.getByRole('button', { name: /^运\s*行$/ }));
    expect(
      screen.getByText('Preview 1: export default {}')
    ).toBeInTheDocument();
    expect(save).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  });

  test('does not expose migration, upgrade, or dependency state UI for legacy rows', () => {
    const source = 'export default function Legacy() { return <div />; }';
    const saveActiveDraft = vi.fn().mockResolvedValue(undefined);
    const activeTab = {
      block_id: block.id,
      detail: { block_id: block.id, tab_id: 'tab-1', title: 'Orders' },
      base_source: source,
      draft: source,
      source_sha256: null,
      executable: {
        block_id: block.id,
        page_id: 'page-1',
        source_code: source,
        source_sha256: null,
        dependency_lock: null,
        tailwind_toolchain_lock: null,
        generated_css: null,
        generated_css_sha256: null,
        compiler_identity: null,
        executable_state: 'legacy' as const
      },
      loading: false,
      saving: false,
      error: null
    };
    blockTabsHook.useFrontstageBlockTabs.mockReturnValue({
      tabs: [activeTab],
      activeBlockId: activeTab.block_id,
      activeTab,
      anyDirty: false,
      openBlock: vi.fn(),
      activateBlock: vi.fn(),
      closeBlock: vi.fn(),
      setDraft: vi.fn(),
      setActiveDraft: vi.fn(),
      resetActive: vi.fn(),
      saveActiveDraft,
      handleDeletedBlock: vi.fn().mockResolvedValue('converged')
    });

    render(
      <FrontstageJsxStudioDrawer
        open
        initialSection="code"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );

    expect(screen.queryByText(/迁移|升级|可执行依赖/u)).not.toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    expect(saveActiveDraft).not.toHaveBeenCalled();
  });

  test('D4-AC-006 preserves controlled legacy source and blocks save/run with the stable diagnostic', () => {
    const legacySource = `async function main(ctx) { return { view: null, outputs: {} }; }
export default { main } satisfies BlockModule;`;
    const save = vi.fn();
    const runPanel = vi.fn(({ runRevision }) => (
      <div>{String(runRevision)}</div>
    ));
    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: legacySource,
      draft: legacySource,
      dirty: false,
      loading: false,
      saving: false,
      error: null,
      permissionDenied: false,
      setDraft: vi.fn(),
      reset: vi.fn(),
      save
    });
    const model = {};
    monacoEditor.getModel.mockReturnValue(model);

    render(
      <FrontstageJsxStudioDrawer
        open
        initialSection="code"
        workspaceId="workspace-1"
        pageId="page-1"
        tabId="tab-1"
        block={block}
        catalogEntry={catalogEntry}
        runPanel={runPanel}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );

    expect(screen.getByRole('textbox', { name: 'JSX source' })).toHaveValue(
      legacySource
    );
    expect(monacoHook.setModelMarkers).toHaveBeenLastCalledWith(
      model,
      expect.any(String),
      [
        expect.objectContaining({
          message: LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC.message
        })
      ]
    );
    expect(screen.queryByText('代码诊断')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /^运\s*行$/ }));
    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    expect(save).not.toHaveBeenCalled();
    expect(runPanel.mock.calls.at(-1)?.[0]).toMatchObject({
      code: legacySource,
      runRevision: null
    });
  });
});
