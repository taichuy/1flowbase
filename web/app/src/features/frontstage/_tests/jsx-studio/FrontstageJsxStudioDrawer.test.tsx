import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { appI18n } from '../../../../shared/i18n/app-i18n';
import { FrontstageJsxStudioDrawer } from '../../components/jsx-studio/FrontstageJsxStudioDrawer';
import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import type { FrontstageBlockInstance } from '../../lib/page-document';

const blockCodeHook = vi.hoisted(() => ({
  useFrontstageBlockCode: vi.fn()
}));
const dataCapabilitiesHook = vi.hoisted(() => ({
  useFrontstageDataCapabilities: vi.fn()
}));

vi.mock('../../hooks/use-frontstage-block-code', () => blockCodeHook);
vi.mock(
  '../../hooks/use-frontstage-data-capabilities',
  () => dataCapabilitiesHook
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
    value,
    onChange,
    options
  }: {
    value?: string;
    onChange?: (value?: string) => void;
    options?: { editContext?: boolean };
  }) => (
    <textarea
      aria-label="JSX source"
      data-edit-context={String(options?.editContext)}
      value={value}
      onChange={(event) => onChange?.(event.target.value)}
    />
  )
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
    dataCapabilitiesHook.useFrontstageDataCapabilities.mockReturnValue({
      data: {
        queries: [
          {
            id: 'frontstage.data_model.record.list',
            kind: 'query',
            params_schema: { type: 'object' },
            result_schema: { type: 'object' }
          }
        ],
        actions: [
          {
            id: 'frontstage.data_model.record.create',
            kind: 'action',
            params_schema: { type: 'object' },
            result_schema: { type: 'object' }
          }
        ],
        models: [
          {
            code: 'orders',
            scope_kind: 'workspace',
            fields: []
          }
        ]
      },
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

    expect(
      screen.getByRole('dialog', { name: 'TSX 编辑器' })
    ).toBeInTheDocument();
    expect(screen.getByLabelText('JSX source')).toBeInTheDocument();
    expect(screen.getByLabelText('JSX source')).toHaveAttribute(
      'data-edit-context',
      'false'
    );
    expect(screen.getByText('结构化配置')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '接口' }));
    expect(
      screen.getByText('frontstage.data_model.record.list')
    ).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole('button', { name: '绑定 orders 查询列表' })
    );

    await waitFor(() => expect(onSaveBlock).toHaveBeenCalledTimes(1));
    expect(onSaveBlock.mock.calls[0]?.[0]).toMatchObject({
      props: {
        title: 'Orders',
        dataBinding: [
          {
            key: 'ordersList',
            id: 'frontstage.data_model.record.list',
            kind: 'query',
            params: { model: 'orders' }
          }
        ]
      }
    });
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

  test('shows generated context in the editor surface and saves code through the existing hook', async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    blockCodeHook.useFrontstageBlockCode.mockReturnValue({
      code: 'export default {}',
      draft: 'export default {}',
      dirty: true,
      loading: false,
      saving: false,
      error: null,
      permissionDenied: false,
      setDraft: vi.fn(),
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
          props: {
            ...block.props,
            dataBinding: [
              {
                key: 'ordersList',
                id: 'frontstage.data_model.record.list',
                kind: 'query',
                params: { model: 'orders' }
              }
            ]
          }
        }}
        catalogEntry={catalogEntry}
        diagnostics={[]}
        onClose={vi.fn()}
        onSaveBlock={vi.fn()}
      />
    );

    expect(screen.getByText('自动注入上下文')).toBeInTheDocument();
    expect(screen.getByText(/ordersList/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '保存代码' }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  });
});
