import { setImmediate } from 'node:timers/promises';

import {
  // eslint-disable-next-line testing-library/no-manual-cleanup -- Explicit teardown drains React scheduler work before jsdom removes window.
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import { appI18n } from '../../../../shared/i18n/app-i18n';
import { LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC } from '@1flowbase/page-runtime';
import { FrontstageJsxStudioDrawer } from '../../components/jsx-studio/FrontstageJsxStudioDrawer';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import type { FrontstageBlockInstance } from '../../lib/page-document';

const blockCodeHook = vi.hoisted(() => ({
  useFrontstageBlockCode: vi.fn()
}));
const interfaceCapabilitiesHook = vi.hoisted(() => ({
  useFrontstageInterfaceCapabilities: vi.fn(),
  useFrontstageInterfaceCapabilityDetails: vi.fn()
}));
const interfaceCapabilitiesApi = vi.hoisted(() => ({
  fetchFrontstageInterfaceCapability: vi.fn()
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

vi.mock('../../hooks/use-frontstage-block-code', () => blockCodeHook);
vi.mock(
  '../../hooks/use-frontstage-interface-capabilities',
  () => interfaceCapabilitiesHook
);
vi.mock('../../api/interface-capabilities', () => interfaceCapabilitiesApi);
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
    <section aria-label={String(title)} role="dialog">
      {extra}
      {children}
    </section>
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
            'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
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

describe('FrontstageJsxStudioDrawer', () => {
  afterEach(async () => {
    cleanup();
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
        diagnostics={[]}
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

    expect(await screen.findByText('接口代码已插入')).toBeInTheDocument();
    expect(onSaveBlock).not.toHaveBeenCalled();
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
        diagnostics={[]}
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
        diagnostics={[]}
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
      diagnostics: [],
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

  test('AC-005 submits snippet and import changes as one Monaco edit batch', () => {
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
        diagnostics={[]}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );
    const row = screen.getByText('Button').closest('div');
    fireEvent.click(within(row!).getByRole('button', { name: '插入代码' }));

    expect(monacoEditor.executeEdits).toHaveBeenCalledTimes(1);
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
        diagnostics={[]}
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
        diagnostics={[]}
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
        diagnostics={[]}
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
        diagnostics={[]}
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
