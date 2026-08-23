import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { App } from 'antd';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import type {
  ConsoleFrontstageComponent,
  ConsoleFrontstageInterfaceCapability
} from '@1flowbase/api-client';

import { appI18n } from '../../../../shared/i18n/app-i18n';
import { JsxStudioResourcePanel } from '../../components/jsx-studio/JsxStudioResourcePanel';
import type { FrontstageJsxEditorProjection } from '../../lib/jsx-studio/editor-projection';
import type { FrontstageJsxInsertion } from '../../lib/jsx-studio/source-insertion';
import type { FrontstageBlockInstance } from '../../lib/page-document';

const interfaceCapabilitiesHook = vi.hoisted(() => ({
  useFrontstageInterfaceCapabilities: vi.fn()
}));
const interfaceCapabilitiesApi = vi.hoisted(() => ({
  fetchFrontstageInterfaceCapability: vi.fn()
}));
const componentCapabilitiesHook = vi.hoisted(() => ({
  useFrontstageComponents: vi.fn()
}));
const clipboard = vi.hoisted(() => ({
  copyTextToClipboard: vi.fn()
}));

vi.mock(
  '../../hooks/use-frontstage-interface-capabilities',
  () => interfaceCapabilitiesHook
);
vi.mock('../../api/interface-capabilities', () => interfaceCapabilitiesApi);
vi.mock(
  '../../hooks/use-frontstage-components',
  () => componentCapabilitiesHook
);
vi.mock('../../../../shared/ui/clipboard/copy-text', () => clipboard);

const block = {
  id: 'orders-block',
  codeRef: 'orders-code',
  ports: { inputs: [], outputs: [] },
  props: {},
  presentation: { heightMode: 'auto', height: null }
} as unknown as FrontstageBlockInstance;

const operations = [
  {
    interface_id:
      'data_model__019f56b6-c8d9-7981-af0b-eb38d1b29393__list_records',
    method: 'GET',
    path: '/api/runtime/models/application_conversations/list',
    name: 'List conversations',
    short_description: 'List conversation records',
    parameter_schema: { type: 'object', properties: {} },
    result_schema: { type: 'object', properties: {} },
    request_media_type: null,
    response_media_type: 'application/json',
    schema_digest: 'digest-list',
    scope: 'frontstage_page_tab',
    risk_level: 'low',
    bindable: true,
    disabled_reason: null
  },
  {
    interface_id: 'get_frontstage_page_detail',
    method: 'GET',
    path: '/api/console/frontstage/pages/{page_id}',
    name: 'Get page detail',
    short_description: 'Read one frontstage page',
    parameter_schema: {
      type: 'object',
      properties: {
        path: {
          type: 'object',
          properties: { page_id: { type: 'string' } },
          required: ['page_id']
        }
      },
      required: ['path']
    },
    result_schema: {
      type: 'object',
      properties: { id: { type: 'string' } },
      required: ['id']
    },
    request_media_type: null,
    response_media_type: 'application/json',
    schema_digest: 'digest-detail',
    scope: 'frontstage_page_tab',
    risk_level: 'low',
    bindable: true,
    disabled_reason: null
  }
] as ConsoleFrontstageInterfaceCapability[];

const projection: FrontstageJsxEditorProjection = {
  contextComment: '',
  allowedImportSources: new Set<string>(),
  monacoExtraLibs: []
};

const surfaceComponent = {
  id: '019c0000-0000-7000-8000-000000000001',
  scope_id: '00000000-0000-0000-0000-000000000000',
  component_code: 'official.surface',
  name: 'Surface',
  description: 'Native React surface with standard DOM props.',
  import_code: "import { Surface } from '@definitely/not-installed';",
  source_code: '<Surface className="card">Content</Surface>',
  origin: 'official',
  source: 'official',
  group: 'layout',
  upstream: { identity: '@definitely/not-installed', version: '99.0.0' },
  version: '1.0.0',
  keywords: ['surface'],
  catalog_updated_at: null,
  source_locator: null,
  source_checksum: null,
  created_at: '2026-08-23T00:00:00Z',
  updated_at: '2026-08-23T00:00:00Z'
} satisfies ConsoleFrontstageComponent;

const createInsertCodeMock = () =>
  vi.fn<(insertion: FrontstageJsxInsertion) => void>();
const createSaveBlockMock = () =>
  vi
    .fn<(nextBlock: FrontstageBlockInstance) => Promise<boolean | void>>()
    .mockResolvedValue(true);

function renderWithApp(children: ReactNode) {
  return render(<App>{children}</App>);
}

function renderInterfacePanel({
  codeSource = '',
  interfacePathPrefixes,
  onInsertCode = createInsertCodeMock(),
  onSaveBlock = createSaveBlockMock()
}: {
  codeSource?: string;
  interfacePathPrefixes?: readonly string[];
  onInsertCode?: ReturnType<typeof createInsertCodeMock>;
  onSaveBlock?: ReturnType<typeof createSaveBlockMock>;
} = {}) {
  renderWithApp(
    <JsxStudioResourcePanel
      block={block}
      codeSource={codeSource}
      interfacePathPrefixes={interfacePathPrefixes}
      pageBlocks={[block]}
      workspaceId="workspace-1"
      projection={projection}
      section="interfaces"
      onInsertCode={onInsertCode}
      onSaveBlock={onSaveBlock}
    />
  );
  return { onInsertCode, onSaveBlock };
}

async function settleGlobalMessage(text: string) {
  expect(await screen.findByText(text)).toBeInTheDocument();
}

describe('TSX Studio interface connector', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await appI18n.changeLanguage('zh_Hans');
    interfaceCapabilitiesHook.useFrontstageInterfaceCapabilities.mockReturnValue(
      {
        data: {
          items: operations.map(
            ({
              interface_id,
              method,
              path,
              adapter_id = 'console_openapi'
            }) => ({
              interface_id,
              method,
              path,
              adapter_id
            })
          ),
          total: operations.length,
          offset: 0,
          limit: 10,
          has_more: false,
          next_offset: null,
          adapter_ids: ['console_openapi', 'runtime_data_model'],
          methods: ['GET']
        },
        loading: false,
        error: null
      }
    );
    interfaceCapabilitiesApi.fetchFrontstageInterfaceCapability.mockImplementation(
      async (_workspaceId: string, interfaceId: string) =>
        operations.find((operation) => operation.interface_id === interfaceId)
    );
  });

  test('passes path scopes to the backend without exposing them in the connector UI', () => {
    renderInterfacePanel({
      interfacePathPrefixes: [
        '/api/public/',
        '/api/console/settings/auth-center/'
      ]
    });

    expect(screen.queryByText('/api/public/')).not.toBeInTheDocument();
    expect(
      screen.queryByText('/api/console/settings/auth-center/')
    ).not.toBeInTheDocument();
    expect(
      interfaceCapabilitiesHook.useFrontstageInterfaceCapabilities
    ).toHaveBeenLastCalledWith(
      'workspace-1',
      expect.objectContaining({
        path_prefixes: ['/api/public/', '/api/console/settings/auth-center/']
      })
    );
    expect(
      screen.getByRole('textbox', { name: '搜索接口路径' })
    ).toBeInTheDocument();
  });

  test('searches paths, loads one detail, and inserts code without saving a binding', async () => {
    const { onInsertCode, onSaveBlock } = renderInterfacePanel();
    expect(
      interfaceCapabilitiesApi.fetchFrontstageInterfaceCapability
    ).not.toHaveBeenCalled();
    fireEvent.change(screen.getByRole('textbox', { name: '搜索接口路径' }), {
      target: { value: '/api/console/frontstage' }
    });
    await waitFor(() =>
      expect(
        interfaceCapabilitiesHook.useFrontstageInterfaceCapabilities
      ).toHaveBeenLastCalledWith(
        'workspace-1',
        expect.objectContaining({ path_query: '/api/console/frontstage' })
      )
    );
    fireEvent.click(
      await screen.findByText('/api/console/frontstage/pages/{page_id}')
    );
    expect(
      interfaceCapabilitiesApi.fetchFrontstageInterfaceCapability
    ).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '插入代码' }));

    await waitFor(() => expect(onInsertCode).toHaveBeenCalledTimes(1));
    await settleGlobalMessage('接口代码已插入');
    expect(
      interfaceCapabilitiesApi.fetchFrontstageInterfaceCapability
    ).toHaveBeenCalledWith('workspace-1', 'get_frontstage_page_detail');
    expect(onSaveBlock).not.toHaveBeenCalled();
    expect(onInsertCode).toHaveBeenCalledWith(
      expect.objectContaining({
        kind: 'source',
        source: expect.stringContaining('const getPageDetail = ('),
        requiredImports: [
          {
            kind: 'type',
            name: 'BlockContext',
            moduleSource: '@1flowbase/block-sdk'
          }
        ]
      })
    );
    expect(onInsertCode.mock.calls[0]?.[0]).toEqual(
      expect.objectContaining({
        source: expect.stringContaining(
          "'/api/console/frontstage/pages/{page_id}'"
        )
      })
    );
  });

  test('inserts a readable callable alias without leaking a dynamic data model id', async () => {
    const { onInsertCode } = renderInterfacePanel({
      codeSource: 'const listApplicationConversations = () => null;'
    });
    fireEvent.click(
      await screen.findByText(
        '/api/runtime/models/application_conversations/list'
      )
    );
    fireEvent.click(screen.getByRole('button', { name: '插入代码' }));

    await waitFor(() => expect(onInsertCode).toHaveBeenCalledTimes(1));
    await settleGlobalMessage('接口代码已插入');
    const insertion = onInsertCode.mock.calls[0]?.[0];
    expect(insertion?.kind).toBe('source');
    const source = insertion?.kind === 'source' ? insertion.source : '';
    expect(source).toContain('const listApplicationConversations2 = (');
    expect(source).not.toContain('019f56b6');
    expect(source).toContain(
      "'/api/runtime/models/application_conversations/list'"
    );
  });

  test('keeps source and method filter menus above the Studio window and applies both filters', async () => {
    renderInterfacePanel();

    fireEvent.mouseDown(screen.getByRole('combobox', { name: '接口来源' }));
    const sourcePopup = document.querySelector<HTMLElement>(
      '.ant-select-dropdown:not(.ant-select-dropdown-hidden)'
    );
    expect(sourcePopup).not.toBeNull();
    expect(
      Number.parseInt(getComputedStyle(sourcePopup!).zIndex, 10)
    ).toBeGreaterThan(1051);
    fireEvent.click(within(sourcePopup!).getByText('数据模型'));

    await waitFor(() =>
      expect(
        interfaceCapabilitiesHook.useFrontstageInterfaceCapabilities
      ).toHaveBeenLastCalledWith(
        'workspace-1',
        expect.objectContaining({ adapter_id: 'runtime_data_model' })
      )
    );

    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Method' }));
    const methodPopup = document.querySelector<HTMLElement>(
      '.ant-select-dropdown:not(.ant-select-dropdown-hidden)'
    );
    expect(methodPopup).not.toBeNull();
    fireEvent.click(
      methodPopup!.querySelector<HTMLElement>('.ant-select-item-option')!
    );

    await waitFor(() =>
      expect(
        interfaceCapabilitiesHook.useFrontstageInterfaceCapabilities
      ).toHaveBeenLastCalledWith(
        'workspace-1',
        expect.objectContaining({
          adapter_id: 'runtime_data_model',
          method: 'GET'
        })
      )
    );
  });

  test('does not insert generated code when capability detail loading fails', async () => {
    const onInsertCode = createInsertCodeMock();
    const onSaveBlock = createSaveBlockMock();
    interfaceCapabilitiesApi.fetchFrontstageInterfaceCapability.mockRejectedValueOnce(
      new Error('detail unavailable')
    );
    renderInterfacePanel({ onInsertCode, onSaveBlock });
    fireEvent.click(
      await screen.findByText('/api/console/frontstage/pages/{page_id}')
    );
    fireEvent.click(screen.getByRole('button', { name: '插入代码' }));

    await waitFor(() =>
      expect(
        interfaceCapabilitiesApi.fetchFrontstageInterfaceCapability
      ).toHaveBeenCalledTimes(1)
    );
    await settleGlobalMessage('接口能力目录加载失败');
    expect(onSaveBlock).not.toHaveBeenCalled();
    expect(onInsertCode).not.toHaveBeenCalled();
  });
});

describe('TSX Studio insertion descriptors', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await appI18n.changeLanguage('zh_Hans');
    componentCapabilitiesHook.useFrontstageComponents.mockReturnValue({
      data: {
        items: [surfaceComponent],
        total: 1,
        offset: 0,
        limit: 10,
        has_more: false,
        next_offset: null
      },
      loading: false,
      error: null
    });
    clipboard.copyTextToClipboard.mockResolvedValue(undefined);
  });

  test('AC-001 describes a context reference instead of assuming a global ctx', () => {
    const onInsertCode = createInsertCodeMock();
    renderWithApp(
      <JsxStudioResourcePanel
        block={block}
        codeSource=""
        pageBlocks={[block]}
        workspaceId="workspace-1"
        projection={projection}
        section="variables"
        onInsertCode={onInsertCode}
        onSaveBlock={createSaveBlockMock()}
      />
    );

    const row = screen.getByText('ctx.currentUser').closest('tr');
    fireEvent.click(within(row!).getByRole('button', { name: '插入代码' }));

    expect(onInsertCode).toHaveBeenCalledWith({
      kind: 'context-reference',
      memberPath: 'currentUser'
    });
  });

  test('AC-022 and AC-023 render registered variables as label, reference, and insert columns', () => {
    const onInsertCode = createInsertCodeMock();
    renderWithApp(
      <JsxStudioResourcePanel
        block={block}
        codeSource=""
        contextVariables={[
          {
            group: 'configuration',
            label: 'Issuer',
            member_path: 'inputs.public_variables.issuer',
            schema: { type: 'string' }
          },
          {
            group: 'runtime',
            label: 'API',
            member_path: 'api',
            schema: { type: 'object' }
          }
        ]}
        pageBlocks={[block]}
        workspaceId="workspace-1"
        projection={projection}
        section="variables"
        onInsertCode={onInsertCode}
        onSaveBlock={createSaveBlockMock()}
      />
    );

    expect(screen.getAllByRole('columnheader', { name: '标签' })).toHaveLength(
      2
    );
    expect(screen.getAllByRole('columnheader', { name: '变量' })).toHaveLength(
      2
    );
    expect(screen.getAllByRole('columnheader', { name: '操作' })).toHaveLength(
      2
    );
    const configurationGroup = screen.getByRole('region', {
      name: '配置变量'
    });
    const runtimeGroup = screen.getByRole('region', {
      name: '运行时上下文'
    });
    expect(within(configurationGroup).getByText('Issuer')).toBeInTheDocument();
    expect(within(runtimeGroup).getByText('API')).toBeInTheDocument();
    const row = screen.getByText('Issuer').closest('tr');
    expect(
      within(row!).getByText('ctx.inputs.public_variables.issuer')
    ).toBeInTheDocument();
    fireEvent.click(within(row!).getByRole('button', { name: '插入代码' }));

    expect(onInsertCode).toHaveBeenCalledWith({
      kind: 'context-reference',
      memberPath: 'inputs.public_variables.issuer'
    });
  });

  test('AC-024 renders an unavailable state instead of Frontstage defaults', () => {
    renderWithApp(
      <JsxStudioResourcePanel
        block={block}
        codeSource=""
        contextVariables={null}
        pageBlocks={[block]}
        workspaceId="workspace-1"
        projection={projection}
        section="variables"
        onInsertCode={createInsertCodeMock()}
        onSaveBlock={createSaveBlockMock()}
      />
    );

    expect(screen.getByText('变量上下文不可用')).toBeInTheDocument();
    expect(screen.queryByText('ctx.currentUser')).not.toBeInTheDocument();
  });

  test('WP-D4 inserts and copies the persisted raw component code exactly', async () => {
    const onInsertCode = createInsertCodeMock();
    renderWithApp(
      <JsxStudioResourcePanel
        block={block}
        codeSource=""
        pageBlocks={[block]}
        workspaceId="workspace-1"
        projection={projection}
        section="components"
        onInsertCode={onInsertCode}
        onSaveBlock={createSaveBlockMock()}
      />
    );

    expect(
      screen.getByRole('columnheader', { name: '组件' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '描述' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '操作' })
    ).toBeInTheDocument();
    expect(screen.getByText(surfaceComponent.description)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '插入' }));

    await waitFor(() =>
      expect(onInsertCode).toHaveBeenCalledWith({
        kind: 'component',
        importCode: surfaceComponent.import_code,
        source: surfaceComponent.source_code
      })
    );

    fireEvent.click(screen.getByRole('button', { name: '复制 API' }));
    await waitFor(() =>
      expect(clipboard.copyTextToClipboard).toHaveBeenCalledWith(
        `${surfaceComponent.import_code}\n\n${surfaceComponent.source_code}`
      )
    );
  });

  test('D2-AC-001 searches the global component directory without constraining it to the active block Catalog', async () => {
    componentCapabilitiesHook.useFrontstageComponents.mockReturnValue({
      data: {
        items: [surfaceComponent],
        total: 21,
        offset: 0,
        limit: 10,
        has_more: true,
        next_offset: 10
      },
      loading: false,
      error: null
    });
    renderWithApp(
      <JsxStudioResourcePanel
        block={block}
        codeSource=""
        pageBlocks={[block]}
        workspaceId="workspace-1"
        projection={projection}
        section="components"
        onInsertCode={createInsertCodeMock()}
        onSaveBlock={createSaveBlockMock()}
      />
    );

    const search = screen.getByRole('searchbox', { name: '搜索组件' });
    fireEvent.change(search, { target: { value: 'surface' } });
    fireEvent.keyDown(search, { key: 'Enter', code: 'Enter' });
    await waitFor(() =>
      expect(
        componentCapabilitiesHook.useFrontstageComponents
      ).toHaveBeenLastCalledWith(
        'workspace-1',
        expect.objectContaining({ query: 'surface', offset: 0, limit: 10 }),
        true
      )
    );
    const [, searchRequest] =
      componentCapabilitiesHook.useFrontstageComponents.mock.lastCall ?? [];
    expect(searchRequest).not.toHaveProperty('installation_id');
    expect(searchRequest).not.toHaveProperty('contribution_code');

    fireEvent.click(screen.getByTitle('2'));
    await waitFor(() =>
      expect(
        componentCapabilitiesHook.useFrontstageComponents
      ).toHaveBeenLastCalledWith(
        'workspace-1',
        expect.objectContaining({ query: 'surface', offset: 10, limit: 10 }),
        true
      )
    );
    const [, pageRequest] =
      componentCapabilitiesHook.useFrontstageComponents.mock.lastCall ?? [];
    expect(pageRequest).not.toHaveProperty('installation_id');
    expect(pageRequest).not.toHaveProperty('contribution_code');
  });
});
