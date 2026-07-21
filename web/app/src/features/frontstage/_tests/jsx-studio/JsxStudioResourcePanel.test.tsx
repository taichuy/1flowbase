import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import type { ConsoleFrontstageCallableInterface } from '@1flowbase/api-client';

import { appI18n } from '../../../../shared/i18n/app-i18n';
import { JsxStudioResourcePanel } from '../../components/jsx-studio/JsxStudioResourcePanel';
import type { FrontstageJsxEditorProjection } from '../../lib/jsx-studio/editor-projection';
import type { FrontstageBlockInstance } from '../../lib/page-document';

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
    operation_id: 'list_application_conversations_records',
    method: 'GET',
    path: '/api/runtime/models/application_conversations/list',
    name: 'List conversations',
    description: 'List conversation records',
    request_schema: { type: 'object', properties: {} },
    response_schema: { type: 'object', properties: {} },
    request_media_type: null,
    response_media_type: 'application/json',
    schema_digest: 'digest-list',
    scope: 'frontstage_page_tab',
    risk_level: 'low',
    bindable: true,
    disabled_reason: null
  },
  {
    operation_id: 'get_frontstage_page_detail',
    method: 'GET',
    path: '/api/frontstage/pages/{page_id}',
    name: 'Get page detail',
    description: 'Read one frontstage page',
    request_schema: {
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
    response_schema: {
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
] as ConsoleFrontstageCallableInterface[];

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
      callableInterfaces={operations}
      callableInterfacesError={null}
      callableInterfacesLoading={false}
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
  });

  test('searches callable interfaces and inserts generated code after binding succeeds', async () => {
    const { onInsertCode, onSaveBlock } = renderInterfacePanel();
    const select = screen.getByRole('combobox', { name: '接口' });

    fireEvent.mouseDown(select);
    fireEvent.change(select, { target: { value: 'page detail' } });
    const option = await screen.findByText(
      'GET /api/frontstage/pages/{page_id}'
    );
    expect(
      select.closest('.frontstage-jsx-studio__resource-section')
    ).toContainElement(option);
    fireEvent.click(option);
    fireEvent.click(screen.getByRole('button', { name: '绑定并插入' }));

    await waitFor(() => expect(onSaveBlock).toHaveBeenCalledTimes(1));
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

  test('does not insert generated code when binding is rejected', async () => {
    const onInsertCode = createInsertCodeMock();
    const onSaveBlock = createSaveBlockMock().mockResolvedValue(false);
    renderInterfacePanel({ onInsertCode, onSaveBlock });

    const select = screen.getByRole('combobox', { name: '接口' });
    fireEvent.mouseDown(select);
    fireEvent.change(select, {
      target: { value: 'get_frontstage_page_detail' }
    });
    fireEvent.click(
      await screen.findByText('GET /api/frontstage/pages/{page_id}')
    );
    fireEvent.click(screen.getByRole('button', { name: '绑定并插入' }));

    await waitFor(() => expect(onSaveBlock).toHaveBeenCalledTimes(1));
    expect(onInsertCode).not.toHaveBeenCalled();
  });
});
