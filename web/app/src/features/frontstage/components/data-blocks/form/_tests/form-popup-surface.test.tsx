import { fireEvent, render, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ConfigProvider, theme } from 'antd';
import { useContext } from 'react';
import { ConfigContext } from 'antd/es/config-provider/context';
import { FRONTSTAGE_DESIGN_BLUE } from '../../../../lib/design-mode-theme';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { BlockContext } from '@1flowbase/page-protocol';
import { NativeBlockSurfaceProvider } from '../../../../lib/native-modules/native-block-surface-context';
import { createNativeOverlayHost } from '../../../../lib/native-modules/native-overlay-host';
import { createNativeBlockSurfaceRuntime } from '../../../../lib/native-modules/surface/native-block-surface-runtime';
import {
  DataBlockHostContext,
  type DataBlockHost
} from '../../../../lib/data-blocks/host-context';
import {
  createFormBlockConfig,
  type DataBlockField,
  type DataFormItem
} from '../../../../lib/data-blocks/types';
import { FormBlock } from '../FormBlock';

const api = vi.hoisted(() => ({ metadata: vi.fn(), records: vi.fn() }));
vi.mock('../../../../api/data-blocks', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../../api/data-blocks')>()),
  useDataBlockMetadata: api.metadata,
  useDataBlockRecords: api.records,
  useDataBlockRecord: () => ({
    data: undefined,
    isLoading: false,
    isError: false
  }),
  useDataBlockWrites: () => ({
    create: { mutateAsync: vi.fn() },
    update: { mutateAsync: vi.fn() }
  })
}));
// A token consumer inside a normal markdown form item observes the local provider.
vi.mock('@ant-design/x-markdown', () => ({
  default: () => {
    const { token } = theme.useToken();
    const config = useContext(ConfigContext);
    return (
      <>
        <output aria-label="form primary color">{token.colorPrimary}</output>
        <output aria-label="form button hover color">
          {config.theme?.components?.Button?.defaultHoverColor}
        </output>
        <output aria-label="form input focus color">
          {config.theme?.components?.Input?.activeBorderColor}
        </output>
      </>
    );
  }
}));

const fields: DataBlockField[] = [
  { id: 'title', code: 'title', title: '客户名称', field_kind: 'string' },
  {
    id: 'status',
    code: 'status',
    title: '状态',
    field_kind: 'enum',
    display_interface: 'select',
    display_options: {
      options: [
        { label: '草稿', value: 'draft' },
        { label: '已完成', value: 'paid' }
      ]
    }
  },
  {
    id: 'scheduled',
    code: 'scheduled',
    title: '预约时间',
    field_kind: 'datetime'
  },
  {
    id: 'customer',
    code: 'customer',
    title: '所属客户',
    field_kind: 'many_to_one',
    relation_target_model_id: 'customers',
    relation_target_model_code: 'customers'
  }
].map((field) => ({
  description: null,
  is_writable: true,
  is_required: false,
  api_required: false,
  default_value: null,
  display_interface: null,
  display_options: {},
  relation_target_model_id: null,
  relation_target_model_code: null,
  relation_options: {},
  ...field
}));
const capabilities = {
  can_list: true,
  can_get: true,
  can_create: true,
  can_update: true,
  can_delete: true
};
const dispose: Array<() => void> = [];
const item = (field_code: string): DataFormItem => ({
  id: field_code,
  kind: 'field',
  field_code,
  row: field_code,
  span: 24
});

function mountForm(
  blockId: string,
  items: DataFormItem[] = [],
  designMode = true
) {
  const element = document.createElement('div');
  document.body.append(element);
  const shadow = element.attachShadow({ mode: 'open' });
  const clippedViewport = document.createElement('div');
  clippedViewport.style.overflow = 'hidden';
  clippedViewport.style.height = '60px';
  shadow.append(clippedViewport);
  const overlayHost = createNativeOverlayHost({ blockId, targetRoot: shadow });
  const runtime = createNativeBlockSurfaceRuntime({
    targetRoot: shadow,
    scrollOwner: window,
    overlayHost,
    layoutEpoch: 'test'
  });
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } }
  });
  const host: DataBlockHost = {
    block_id: blockId,
    design_mode: designMode,
    saving: false,
    targets: [],
    saveConfig: vi.fn().mockResolvedValue(undefined),
    prepareAction: vi.fn(),
    openAction: vi.fn(),
    close: vi.fn()
  };
  const config = {
    ...createFormBlockConfig({
      model_id: 'orders',
      model_code: 'orders',
      data_source_id: null
    }),
    items
  };
  const ctx = {
    workspace: { id: 'workspace' },
    currentUser: { id: 'user', permissions: [] },
    props: { data_block: config },
    inputs: {},
    outputs: { publish: vi.fn() }
  } as unknown as BlockContext;
  const view = render(
    <ConfigProvider
      theme={{
        token: { motion: false, colorPrimary: '#008855', blue: '#123abc' },
        components: {
          Button: { defaultHoverColor: '#008855' },
          Input: { activeBorderColor: '#008855' }
        }
      }}
    >
      <QueryClientProvider client={queryClient}>
        <NativeBlockSurfaceProvider scope={runtime}>
          <DataBlockHostContext.Provider value={host}>
            <FormBlock ctx={ctx} />
          </DataBlockHostContext.Provider>
        </NativeBlockSurfaceProvider>
      </QueryClientProvider>
    </ConfigProvider>,
    { container: clippedViewport }
  );
  dispose.push(() => {
    view.unmount();
    runtime.dispose();
    overlayHost.dispose();
    queryClient.clear();
    element.remove();
  });
  return {
    root: within(clippedViewport),
    overlay: within(overlayHost.container),
    clippedViewport,
    overlayNode: overlayHost.container,
    host
  };
}

beforeEach(() => {
  const metadata = {
    record_id_field: 'id',
    fields,
    record_capabilities: capabilities
  };
  api.metadata.mockImplementation(() => ({
    data: metadata,
    isLoading: false,
    isError: false,
    refetch: vi.fn()
  }));
  api.records.mockReturnValue({
    data: { items: [{ id: 'customer-1', title: '关联客户' }], total: 1 },
    isLoading: false,
    isError: false
  });
});
afterEach(() => {
  dispose
    .splice(0)
    .reverse()
    .forEach((cleanup) => cleanup());
});

describe('FormBlock popup ownership', () => {
  it('mounts the real field menu in its own native top layer outside the clipped viewport', async () => {
    const first = mountForm('first');
    const second = mountForm('second');
    fireEvent.click(first.root.getByText('配置字段', { exact: true }));
    const choice = await first.overlay.findByRole('menuitem', {
      name: '客户名称'
    });
    expect(first.clippedViewport.contains(choice)).toBe(false);
    expect(first.overlayNode).toHaveAttribute('popover', 'manual');
    await waitFor(() =>
      expect(first.overlayNode).toHaveAttribute(
        'data-flowbase-native-overlay-state',
        'open'
      )
    );
    expect(
      second.overlay.queryByRole('menuitem', { name: '客户名称' })
    ).not.toBeInTheDocument();
    fireEvent.click(choice);
    await waitFor(() =>
      expect(first.host.saveConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          items: [expect.objectContaining({ field_code: 'title' })]
        })
      )
    );
    expect(second.host.saveConfig).not.toHaveBeenCalled();
    expect(await first.root.findByLabelText('客户名称')).toBeInTheDocument();
  });

  it.each([
    { code: 'status', label: '状态', choice: '已完成' },
    { code: 'customer', label: '所属客户', choice: '关联客户 (customer-1)' }
  ])(
    'keeps the $code selection popup in the owning surface',
    async ({ code, label, choice }) => {
      const form = mountForm(code, [item(code)]);
      fireEvent.mouseDown(form.root.getByLabelText(label));
      const option = await form.overlay.findByText(choice, { exact: true });
      expect(form.clippedViewport.contains(option)).toBe(false);
      fireEvent.click(option);
      expect(form.root.getByText(choice, { exact: true })).toBeInTheDocument();
    }
  );

  it('mounts the date picker calendar outside the field viewport', async () => {
    const form = mountForm('date', [item('scheduled')]);
    fireEvent.click(form.root.getByLabelText('预约时间'));
    await waitFor(() => {
      expect(
        form.overlayNode.querySelector('[class*="picker-dropdown"]')
      ).not.toBeNull();
      expect(
        form.clippedViewport.querySelector('[class*="picker-dropdown"]')
      ).toBeNull();
    });
  });

  it('lets default Select controls inside settings modals inherit the same popup owner', async () => {
    const form = mountForm('modal');
    fireEvent.click(form.root.getByRole('button', { name: '表单区块设置' }));
    fireEvent.click(
      await form.overlay.findByRole('menuitem', { name: '父子记录关联' })
    );
    const dialog = await form.overlay.findByRole('dialog', {
      name: '父子记录关联'
    });
    fireEvent.mouseDown(within(dialog).getByRole('combobox'));
    const option = await form.overlay.findByText('所属客户', { exact: true });
    expect(dialog.contains(option)).toBe(false);
    expect(form.clippedViewport.contains(option)).toBe(false);
    fireEvent.click(option);
    expect(
      within(dialog).getByText('所属客户', { exact: true })
    ).toBeInTheDocument();
  });

  it('uses tool blue only in design mode and preserves the runtime brand color', () => {
    const markdown: DataFormItem = {
      id: 'markdown',
      kind: 'markdown',
      text: 'Text',
      row: 'markdown',
      span: 24
    };
    const design = mountForm('design-theme', [markdown], true);
    const runtime = mountForm('runtime-theme', [markdown], false);
    expect(design.root.getByLabelText('form primary color')).toHaveTextContent(
      FRONTSTAGE_DESIGN_BLUE.primary
    );
    expect(runtime.root.getByLabelText('form primary color')).toHaveTextContent(
      '#008855'
    );
    expect(
      design.root.getByLabelText('form button hover color')
    ).toHaveTextContent(FRONTSTAGE_DESIGN_BLUE.primaryStrong);
    expect(
      runtime.root.getByLabelText('form button hover color')
    ).toHaveTextContent('#008855');
    expect(
      design.root.getByLabelText('form input focus color')
    ).toHaveTextContent(FRONTSTAGE_DESIGN_BLUE.primary);
    expect(
      runtime.root.getByLabelText('form input focus color')
    ).toHaveTextContent('#008855');
  });
});
