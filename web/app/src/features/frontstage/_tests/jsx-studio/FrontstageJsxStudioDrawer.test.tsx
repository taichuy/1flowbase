import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { appI18n } from '../../../../shared/i18n/app-i18n';
import { FrontstageJsxStudioDrawer } from '../../components/jsx-studio/FrontstageJsxStudioDrawer';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import type { FrontstageBlockInstance } from '../../lib/page-document';

const blockCodeHook = vi.hoisted(() => ({
  useFrontstageBlockCode: vi.fn()
}));
const callableInterfacesHook = vi.hoisted(() => ({
  useFrontstageCallableInterfaces: vi.fn()
}));
const monacoHook = vi.hoisted(() => ({
  addExtraLib: vi.fn(),
  setCompilerOptions: vi.fn()
}));

vi.mock('../../hooks/use-frontstage-block-code', () => blockCodeHook);
vi.mock(
  '../../hooks/use-frontstage-callable-interfaces',
  () => callableInterfacesHook
);
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
    options
  }: {
    beforeMount?: (monaco: unknown) => void;
    value?: string;
    onChange?: (value?: string) => void;
    options?: { editContext?: boolean };
  }) => {
    beforeMount?.({
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
    });
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
  interfaces: [],
  presentation: { heightMode: 'auto', height: null },
  layout: { order: 0 },
  order: 0,
  runtime: { kind: 'iframe', entry: 'index.js', hint: 'iframe' }
};

const catalogEntry: NormalizedFrontstageBlockCatalogEntry = {
  id: '1flowbase:frontstage.js-ui-block',
  runtimeKind: 'iframe',
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
  codeCapabilities: {
    template: null,
    allowedImports: ['@1flowbase/block-renderer/antd-facade'],
    monacoExtraLibs: [
      {
        filePath: 'file:///node_modules/antd-facade/index.d.ts',
        content:
          "declare module '@1flowbase/block-renderer/antd-facade' { export const Stack: unknown; }"
      }
    ],
    workerModuleSources: ['@1flowbase/block-renderer/antd-facade']
  },
  raw: {} as NormalizedFrontstageBlockCatalogEntry['raw']
};

describe('FrontstageJsxStudioDrawer', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
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
    callableInterfacesHook.useFrontstageCallableInterfaces.mockReturnValue({
      data: [
        {
          operation_id: 'list_application_conversations_records',
          method: 'GET',
          path: '/api/runtime/models/application_conversations/list',
          name: 'List conversations',
          description: 'List conversations',
          parameters: [],
          request_schema: { type: 'object', properties: {} },
          response_schema: { type: 'object', properties: {} },
          schema_digest: 'digest-1',
          adapter_id: 'runtime_data_model',
          host_injected_parameters: [],
          scope: 'frontstage_page_tab',
          risk_level: 'low',
          authorization: 'runtime_scope_grant_and_page_tab_access',
          bindable: true,
          disabled_reason: null
        }
      ],
      loading: false,
      error: null
    });
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
    expect(screen.getByText('区块设置')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '接口' }));
    expect(
      screen.getByText('list_application_conversations_records')
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /绑\s*定/ }));

    await waitFor(() => expect(onSaveBlock).toHaveBeenCalledTimes(1));
    expect(onSaveBlock.mock.calls[0]?.[0]).toMatchObject({
      interfaces: [
        expect.objectContaining({
          operation_id: 'list_application_conversations_records',
          schema_digest: 'digest-1'
        })
      ]
    });
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

  test('injects generated context from the window header and saves code through the existing hook', async () => {
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
        block={{
          ...block,
          interfaces: []
        }}
        catalogEntry={catalogEntry}
        diagnostics={[]}
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
    fireEvent.click(
      within(windowHeader as HTMLElement).getByRole('button', {
        name: '注入上下文'
      })
    );
    expect(setDraft).toHaveBeenCalledWith(
      expect.stringContaining('@1flowbase-context')
    );
    fireEvent.click(screen.getByRole('button', { name: '保存代码' }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  });
});
