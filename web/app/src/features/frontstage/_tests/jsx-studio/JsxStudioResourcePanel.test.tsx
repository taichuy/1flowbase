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
  interfaces: [],
  ports: { inputs: [], outputs: [] },
  props: {},
  presentation: { heightMode: 'auto', height: null }
} as unknown as FrontstageBlockInstance;

const operations = [
  {
    interface_id: 'list_application_conversations_records',
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
  bindings: [],
  components: [],
  contextComment: '',
  monacoExtraLibs: []
};

const createInsertCodeMock = () => vi.fn<(source: string) => void>();
const createSaveBlockMock = () =>
  vi
    .fn<(nextBlock: FrontstageBlockInstance) => Promise<boolean | void>>()
    .mockResolvedValue(true);

function renderInterfacePanel({
  onInsertCode = createInsertCodeMock(),
  onSaveBlock = createSaveBlockMock()
}: {
  onInsertCode?: ReturnType<typeof createInsertCodeMock>;
  onSaveBlock?: ReturnType<typeof createSaveBlockMock>;
} = {}) {
  render(
    <JsxStudioResourcePanel
      block={block}
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

  test('searches paths through the backend page and loads detail before binding', async () => {
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
    fireEvent.click(screen.getByRole('button', { name: '绑定并插入' }));

    await waitFor(() => expect(onSaveBlock).toHaveBeenCalledTimes(1));
    expect(
      interfaceCapabilitiesApi.fetchFrontstageInterfaceCapability
    ).toHaveBeenCalledWith('workspace-1', 'get_frontstage_page_detail');
    expect(onSaveBlock.mock.calls[0]?.[0]).toMatchObject({
      interfaces: [
        expect.objectContaining({
          alias: 'getFrontstagePageDetail',
          operation_id: 'get_frontstage_page_detail',
          schema_digest: 'digest-detail'
        })
      ]
    });
    expect(onInsertCode).toHaveBeenCalledTimes(1);
    expect(onInsertCode).toHaveBeenCalledWith(
      expect.stringContaining('async function getFrontstagePageDetail(')
    );
    expect(onSaveBlock.mock.invocationCallOrder[0]).toBeLessThan(
      onInsertCode.mock.invocationCallOrder[0]
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

  test('does not insert generated code when binding is rejected', async () => {
    const onInsertCode = createInsertCodeMock();
    const onSaveBlock = createSaveBlockMock().mockResolvedValue(false);
    renderInterfacePanel({ onInsertCode, onSaveBlock });
    fireEvent.click(
      await screen.findByText('/api/console/frontstage/pages/{page_id}')
    );
    fireEvent.click(screen.getByRole('button', { name: '绑定并插入' }));

    await waitFor(() => expect(onSaveBlock).toHaveBeenCalledTimes(1));
    expect(onInsertCode).not.toHaveBeenCalled();
  });
});
