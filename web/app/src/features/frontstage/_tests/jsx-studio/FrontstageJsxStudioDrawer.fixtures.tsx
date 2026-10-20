import {
  act,
  fireEvent,
  render as testingRender,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { App } from 'antd';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactElement, ReactNode } from 'react';
import { beforeEach, expect, test, vi } from 'vitest';

import type { ConsoleFrontstageComponent } from '@1flowbase/api-client';
import {
  LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC,
  sha256Text
} from '@1flowbase/page-runtime';

import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../shared/i18n/app-i18n';
import { FrontstageJsxStudioDrawer } from '../../components/jsx-studio/FrontstageJsxStudioDrawer';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import type { FrontstageBlockInstance } from '../../lib/page-document';

async function renderSettled(ui: ReactElement) {
  let view!: ReturnType<typeof testingRender>;
  await act(async () => {
    view = testingRender(ui, {
      wrapper: ({ children }) => (
        <QueryClientProvider
          client={
            new QueryClient({ defaultOptions: { queries: { retry: false } } })
          }
        >
          <App>{children}</App>
        </QueryClientProvider>
      )
    });
  });
  return view;
}
const templateApi = vi.hoisted(() => ({
  createSettingsUiTemplate: vi.fn(),
  settingsUiTemplatesQueryKey: ['settings', 'ui-management', 'templates']
}));
const templateAuth = vi.hoisted(() => ({
  csrfToken: 'csrf-test',
  actor: null,
  me: { permissions: ['settings_feature.access.system.ui-management'] }
}));
vi.mock('../../../settings/api/ui-management', () => templateApi);
vi.mock('../../../../state/auth-store', () => ({
  useAuthStore: (selector: (state: typeof templateAuth) => unknown) =>
    selector(templateAuth)
}));

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
  useFrontstageComponents: vi.fn()
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
  '../../hooks/use-frontstage-components',
  () => componentCapabilitiesHook
);
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
vi.mock('../../../../shared/code-block/monaco-runtime', () => ({
  loadMonacoEditorModule: () => import('@monaco-editor/react')
}));
vi.mock('@monaco-editor/react', async () => {
  const { useEffect, useRef, useState } = await import('react');
  return {
    default: function MockMonacoEditor({
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
    }) {
      const [mount] = useState(() => {
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
        const editor = monacoEditor;
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
          aria-label="JSX source"
          data-edit-context={String(options?.editContext)}
          value={value}
          onChange={(event) => onChange?.(event.target.value)}
        />
      );
    }
  };
});

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
  codeCapabilities: { template: null },
  raw: {} as NormalizedFrontstageBlockCatalogEntry['raw']
};

const buttonComponent = {
  id: '019c0000-0000-7000-8000-000000000002',
  scope_id: '00000000-0000-0000-0000-000000000000',
  component_code: 'official.button',
  name: 'Button',
  description: 'Native button component.',
  import_code: "import { Button } from '@1flowbase/native-components';",
  source_code: '<Button>Action</Button>',
  origin: 'official',
  source: 'official',
  group: 'inputs',
  upstream: { identity: '@1flowbase/native-components', version: '1.0.0' },
  version: '1.0.0',
  keywords: ['button'],
  catalog_updated_at: null,
  source_locator: null,
  source_checksum: null,
  created_at: '2026-08-23T00:00:00Z',
  updated_at: '2026-08-23T00:00:00Z'
} satisfies ConsoleFrontstageComponent;

beforeEach(async () => {
  vi.clearAllMocks();
  templateAuth.me.permissions = [
    'settings_feature.access.system.ui-management'
  ];
  templateApi.createSettingsUiTemplate.mockResolvedValue({
    id: 'saved-template',
    published_revision: null
  });
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
  await loadApplicationI18nResources();
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
  interfaceCapabilitiesHook.useFrontstageInterfaceCapabilities.mockReturnValue({
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
  });
  interfaceCapabilitiesApi.fetchFrontstageInterfaceCapability.mockResolvedValue(
    capability
  );
  componentCapabilitiesHook.useFrontstageComponents.mockReturnValue({
    data: {
      items: [buttonComponent],
      total: 1,
      offset: 0,
      limit: 10,
      has_more: false,
      next_offset: null
    },
    loading: false,
    error: null
  });
});

async function dispatch<T>(event: () => T) {
  let result!: T;
  await act(async () => {
    result = event();
  });
  return result;
}

const domEvents = {
  click: (...args: Parameters<typeof fireEvent.click>) =>
    dispatch(() => fireEvent.click(...args)),
  change: (...args: Parameters<typeof fireEvent.change>) =>
    dispatch(() => fireEvent.change(...args)),
  mouseDown: (...args: Parameters<typeof fireEvent.mouseDown>) =>
    dispatch(() => fireEvent.mouseDown(...args)),
  mouseMove: (...args: Parameters<typeof fireEvent.mouseMove>) =>
    dispatch(() => fireEvent.mouseMove(...args)),
  mouseUp: (...args: Parameters<typeof fireEvent.mouseUp>) =>
    dispatch(() => fireEvent.mouseUp(...args)),
  keyDown: (...args: Parameters<typeof fireEvent.keyDown>) =>
    dispatch(() => fireEvent.keyDown(...args))
};

export {
  FrontstageJsxStudioDrawer,
  LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC,
  antdAppMocks,
  block,
  blockCodeHook,
  blockTabsHook,
  catalogEntry,
  domEvents,
  expect,
  interfaceCapabilitiesHook,
  monacoEditor,
  monacoHook,
  renderSettled,
  screen,
  templateApi,
  templateAuth,
  test,
  uiTemplatesHook,
  vi,
  waitFor,
  within
};
