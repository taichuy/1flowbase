import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import type { ConsoleFrontstageInterfaceCapability } from '@1flowbase/api-client';

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

vi.mock(
  '../../hooks/use-frontstage-interface-capabilities',
  () => interfaceCapabilitiesHook
);
vi.mock('../../api/interface-capabilities', () => interfaceCapabilitiesApi);

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
  components: [],
  contextComment: '',
  monacoExtraLibs: []
};

const createInsertCodeMock = () =>
  vi.fn<(insertion: FrontstageJsxInsertion) => void>();
const createSaveBlockMock = () =>
  vi
    .fn<(nextBlock: FrontstageBlockInstance) => Promise<boolean | void>>()
    .mockResolvedValue(true);

function renderInterfacePanel({
  codeSource = '',
  onInsertCode = createInsertCodeMock(),
  onSaveBlock = createSaveBlockMock()
}: {
  codeSource?: string;
  onInsertCode?: ReturnType<typeof createInsertCodeMock>;
  onSaveBlock?: ReturnType<typeof createSaveBlockMock>;
} = {}) {
  render(
    <JsxStudioResourcePanel
      block={block}
      codeSource={codeSource}
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
    expect(onSaveBlock).not.toHaveBeenCalled();
    expect(onInsertCode).not.toHaveBeenCalled();
  });
});

describe('TSX Studio insertion descriptors', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await appI18n.changeLanguage('zh_Hans');
  });

  test('AC-001 describes a context reference instead of assuming a global ctx', () => {
    const onInsertCode = createInsertCodeMock();
    render(
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

    const row = screen.getByText('ctx.currentUser').closest('div');
    fireEvent.click(within(row!).getByRole('button', { name: '插入代码' }));

    expect(onInsertCode).toHaveBeenCalledWith({
      kind: 'context-reference',
      memberPath: 'currentUser'
    });
  });

  test('AC-002 carries the catalog module source with a component insertion', () => {
    const onInsertCode = createInsertCodeMock();
    render(
      <JsxStudioResourcePanel
        block={block}
        codeSource=""
        pageBlocks={[block]}
        workspaceId="workspace-1"
        projection={{
          ...projection,
          components: [
            {
              name: 'Button',
              moduleSource: '@1flowbase/block-renderer/antd-facade'
            }
          ]
        }}
        section="components"
        onInsertCode={onInsertCode}
        onSaveBlock={createSaveBlockMock()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '插入代码' }));

    expect(onInsertCode).toHaveBeenCalledWith({
      kind: 'component',
      name: 'Button',
      moduleSource: '@1flowbase/block-renderer/antd-facade'
    });
  });
});
