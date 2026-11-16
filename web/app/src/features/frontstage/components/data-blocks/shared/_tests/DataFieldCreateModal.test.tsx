import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ConfigProvider } from 'antd';
import enUS from 'antd/locale/en_US';
import { useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { BlockContext } from '@1flowbase/page-protocol';
import type {
  ConsoleDataModel,
  ConsoleDataModelField,
  RuntimeDataModelMetadata
} from '@1flowbase/api-client';
import {
  resetAuthStore,
  useAuthStore
} from '../../../../../../state/auth-store';
import {
  dataBlockMetadataKey,
  useDataBlockMetadata
} from '../../../../api/data-blocks';
import { DataFieldCreateModal } from '../DataFieldCreateModal';

const api = vi.hoisted(() => ({
  createConsoleDataModelField: vi.fn(),
  fetchConsoleDataModels: vi.fn(),
  fetchRuntimeDataModelMetadata: vi.fn()
}));
vi.mock('@1flowbase/api-client', async (original) => ({
  ...(await original<typeof import('@1flowbase/api-client')>()),
  ...api
}));
vi.mock('../../../../../../shared/i18n/text', () => ({
  i18nText: (_namespace: string, key: string) => key
}));

const binding = {
  model_id: 'orders-model',
  model_code: 'orders',
  data_source_id: null
};
const recordCapabilities = {
  can_list: true,
  can_get: true,
  can_create: true,
  can_update: true,
  can_delete: true
};
const field: ConsoleDataModelField = {
  id: 'new-field-id',
  code: 'new_field',
  title: 'New field',
  description: null,
  physical_column_name: 'new_field',
  external_field_key: null,
  field_kind: 'string',
  is_system: false,
  is_writable: true,
  is_required: false,
  api_required: false,
  is_unique: false,
  default_value: null,
  display_interface: null,
  display_options: {},
  relation_target_model_id: null,
  relation_options: {},
  sort_order: 0,
  capabilities: {
    ownership: 'user_added',
    can_update_presentation_metadata: true,
    can_update_physical_metadata: true,
    can_delete: true
  }
};
function model(
  fields: ConsoleDataModelField[] = [],
  canAdd = true
): ConsoleDataModel {
  return {
    id: binding.model_id,
    code: binding.model_code,
    title: 'Orders',
    data_source_id: null,
    scope_kind: 'workspace',
    scope_id: 'workspace-1',
    description: null,
    status: 'published',
    runtime_availability: 'available',
    source_kind: 'main_source',
    external_resource_key: null,
    external_table_id: null,
    template_provider: '',
    template_code: '',
    template_version: '',
    template_summary: null,
    physical_table_name: 'orders',
    acl_namespace: 'model',
    audit_namespace: 'model',
    builtin_kind: null,
    capabilities: {
      can_delete: true,
      can_add_user_field: canAdd,
      can_update_lifecycle_status: true,
      record: recordCapabilities
    },
    fields
  };
}
function metadata(
  fields: ConsoleDataModelField[] = []
): RuntimeDataModelMetadata {
  return {
    model_id: binding.model_id,
    model_code: binding.model_code,
    scope_kind: 'workspace',
    scope_id: 'workspace-1',
    record_id_field: 'id',
    record_capabilities: recordCapabilities,
    fields: fields.map((value) => ({
      id: value.id,
      code: value.code,
      title: value.title,
      description: value.description,
      field_kind: value.field_kind,
      is_required: value.is_required,
      api_required: value.api_required,
      is_writable: value.is_writable,
      default_value: value.default_value,
      display_interface: value.display_interface,
      display_options: value.display_options,
      relation_target_model_id: value.relation_target_model_id,
      relation_target_model_code: null,
      relation_options: value.relation_options
    }))
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
const clients: QueryClient[] = [];
function mount(
  options: {
    permission?: boolean;
    onCreated?: (code: string) => void | Promise<void>;
  } = {}
) {
  const ctx = {
    workspace: { id: 'workspace-1' },
    currentUser: {
      id: 'user-1',
      isRoot: false,
      permissions: options.permission === false ? [] : ['model_fields.create']
    },
    props: {},
    inputs: {},
    outputs: { publish: vi.fn() }
  } as unknown as BlockContext;
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0, refetchOnWindowFocus: false },
      mutations: { retry: false }
    }
  });
  clients.push(client);
  const onCreated = vi.fn(options.onCreated ?? (async () => {}));
  const onClose = vi.fn();
  function MetadataConsumer() {
    const result = useDataBlockMetadata(ctx, binding);
    return (
      <output aria-label="Metadata field count">
        {result.data?.fields.length ?? 'pending'}
      </output>
    );
  }
  function Owner() {
    const [open, setOpen] = useState(true);
    return (
      <>
        <MetadataConsumer />
        <DataFieldCreateModal
          ctx={ctx}
          binding={binding}
          open={open}
          onCreated={onCreated}
          onClose={() => {
            onClose();
            setOpen(false);
          }}
        />
      </>
    );
  }
  render(
    <ConfigProvider locale={enUS} theme={{ token: { motion: false } }}>
      <QueryClientProvider client={client}>
        <Owner />
      </QueryClientProvider>
    </ConfigProvider>
  );
  return { client, ctx, onCreated, onClose };
}
async function ready() {
  const dialog = await screen.findByRole('dialog', {
    name: 'data_blocks.fields.create'
  });
  await waitFor(() =>
    expect(
      screen.getByRole('status', { name: 'Metadata field count' })
    ).toHaveTextContent('0')
  );
  return { dialog, ok: within(dialog).getByRole('button', { name: 'OK' }) };
}
async function fill(
  user: ReturnType<typeof userEvent.setup>,
  dialog: HTMLElement
) {
  await user.type(
    within(dialog).getByRole('textbox', { name: 'data_blocks.fields.title' }),
    field.title
  );
  await user.type(
    within(dialog).getByRole('textbox', { name: 'data_blocks.fields.code' }),
    field.code
  );
}
beforeEach(() => {
  vi.clearAllMocks();
  resetAuthStore();
  useAuthStore.setState({ csrfToken: 'csrf-123' });
  api.createConsoleDataModelField.mockResolvedValue(field);
  api.fetchConsoleDataModels
    .mockReset()
    .mockResolvedValue([model([field])])
    .mockResolvedValueOnce([model()]);
  api.fetchRuntimeDataModelMetadata
    .mockReset()
    .mockResolvedValue(metadata([field]))
    .mockResolvedValueOnce(metadata());
});
afterEach(() => {
  cleanup();
  clients.splice(0).forEach((client) => client.clear());
  resetAuthStore();
});

describe('DataFieldCreateModal submission boundaries', () => {
  it('does not validate or write when CSRF preflight is denied', async () => {
    useAuthStore.setState({ csrfToken: null });
    const owner = mount();
    const user = userEvent.setup();
    const { dialog, ok } = await ready();
    await waitFor(() => expect(ok).toBeEnabled());
    await user.click(ok);
    expect(
      within(dialog).getByRole('textbox', { name: 'data_blocks.fields.title' })
    ).not.toHaveAttribute('aria-invalid', 'true');
    expect(
      within(dialog).getByRole('textbox', { name: 'data_blocks.fields.code' })
    ).not.toHaveAttribute('aria-invalid', 'true');
    expect(within(dialog).queryAllByRole('alert')).toHaveLength(0);
    expect(api.createConsoleDataModelField).not.toHaveBeenCalled();
    expect(api.fetchConsoleDataModels).toHaveBeenCalledTimes(1);
    expect(api.fetchRuntimeDataModelMetadata).toHaveBeenCalledTimes(1);
    expect(owner.onCreated).not.toHaveBeenCalled();
    expect(owner.onClose).not.toHaveBeenCalled();
  });

  it.each(['permission', 'model capability'] as const)(
    'does not validate or write when %s denies creation',
    async (denial) => {
      if (denial === 'model capability')
        api.fetchConsoleDataModels
          .mockReset()
          .mockResolvedValue([model([], false)]);
      const owner = mount({ permission: denial !== 'permission' });
      const { dialog, ok } = await ready();
      await screen.findByText('data_blocks.fields.not_allowed');
      expect(ok).toBeDisabled();
      fireEvent.click(ok);
      expect(
        within(dialog).getByRole('textbox', { name: 'data_blocks.fields.code' })
      ).not.toHaveAttribute('aria-invalid', 'true');
      expect(api.createConsoleDataModelField).not.toHaveBeenCalled();
      expect(owner.onCreated).not.toHaveBeenCalled();
      expect(owner.onClose).not.toHaveBeenCalled();
    }
  );

  it('starts both independent refreshes before either settles and attaches only after both', async () => {
    const metadataRefresh = deferred<RuntimeDataModelMetadata>();
    const modelsRefresh = deferred<ConsoleDataModel[]>();
    api.fetchRuntimeDataModelMetadata
      .mockReset()
      .mockResolvedValueOnce(metadata())
      .mockReturnValueOnce(metadataRefresh.promise);
    api.fetchConsoleDataModels
      .mockReset()
      .mockResolvedValueOnce([model()])
      .mockReturnValueOnce(modelsRefresh.promise);
    const owner = mount();
    const user = userEvent.setup();
    const { dialog, ok } = await ready();
    await waitFor(() => expect(ok).toBeEnabled());
    await fill(user, dialog);
    try {
      await user.click(ok);
      await waitFor(() =>
        expect(api.fetchRuntimeDataModelMetadata).toHaveBeenCalledTimes(2)
      );
      expect(api.createConsoleDataModelField).toHaveBeenCalledExactlyOnceWith(
        binding.model_id,
        {
          code: field.code,
          title: field.title,
          description: null,
          field_kind: 'string',
          is_required: false,
          is_unique: false,
          default_value: null,
          external_field_key: null,
          display_interface: null,
          display_options: {},
          relation_target_model_id: null,
          relation_options: {}
        },
        'csrf-123',
        expect.any(String)
      );
      expect(
        within(dialog).getByRole('textbox', { name: 'data_blocks.fields.code' })
      ).toBeDisabled();
      fireEvent.click(ok);
      expect(api.createConsoleDataModelField).toHaveBeenCalledTimes(1);
      // The runtime metadata refresh is deliberately still pending here.
      expect(api.fetchConsoleDataModels).toHaveBeenCalledTimes(2);
      expect(owner.onCreated).not.toHaveBeenCalled();
      await act(async () => {
        modelsRefresh.resolve([model([field])]);
      });
      expect(owner.onCreated).not.toHaveBeenCalled();
      expect(owner.onClose).not.toHaveBeenCalled();
      await act(async () => {
        metadataRefresh.resolve(metadata([field]));
      });
      await waitFor(() =>
        expect(owner.onCreated).toHaveBeenCalledExactlyOnceWith(field.code)
      );
      expect(
        owner.client.getQueryData(
          dataBlockMetadataKey(owner.ctx, binding.model_code)
        )
      ).toEqual(metadata([field]));
      expect(
        owner.client.getQueryData([
          'frontstage-design-models',
          'workspace-1',
          'user-1'
        ])
      ).toEqual([model([field])]);
      await waitFor(() => expect(owner.onClose).toHaveBeenCalledTimes(1));
    } finally {
      await act(async () => {
        metadataRefresh.resolve(metadata([field]));
        modelsRefresh.resolve([model([field])]);
      });
      await waitFor(() => expect(owner.onClose).toHaveBeenCalledTimes(1));
    }
  });

  it('retains real validation and can submit corrected values after validation fails', async () => {
    const owner = mount();
    const user = userEvent.setup();
    const { dialog, ok } = await ready();
    await waitFor(() => expect(ok).toBeEnabled());
    await user.click(ok);
    await waitFor(() =>
      expect(
        within(dialog).getByRole('textbox', { name: 'data_blocks.fields.code' })
      ).toHaveAttribute('aria-invalid', 'true')
    );
    expect(api.createConsoleDataModelField).not.toHaveBeenCalled();
    await fill(user, dialog);
    await user.click(ok);
    await waitFor(() =>
      expect(owner.onCreated).toHaveBeenCalledWith(field.code)
    );
    expect(api.createConsoleDataModelField).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(owner.onClose).toHaveBeenCalledTimes(1));
  });

  it('retries attachment using the already created field without creating it again', async () => {
    const attach = vi
      .fn()
      .mockRejectedValueOnce(new Error('attachment failed'))
      .mockResolvedValueOnce(undefined);
    const owner = mount({ onCreated: attach });
    const user = userEvent.setup();
    const { dialog, ok } = await ready();
    await waitFor(() => expect(ok).toBeEnabled());
    await fill(user, dialog);
    await user.click(ok);
    await screen.findByText('data_blocks.fields.created_attach_failed');
    expect(owner.onClose).not.toHaveBeenCalled();
    expect(
      within(dialog).getByRole('textbox', { name: 'data_blocks.fields.code' })
    ).toHaveValue(field.code);
    expect(
      within(dialog).getByRole('textbox', { name: 'data_blocks.fields.code' })
    ).toBeDisabled();
    await user.click(ok);
    await waitFor(() => expect(owner.onClose).toHaveBeenCalledTimes(1));
    expect(api.createConsoleDataModelField).toHaveBeenCalledTimes(1);
    expect(attach.mock.calls).toEqual([[field.code], [field.code]]);
    expect(api.fetchConsoleDataModels).toHaveBeenCalledTimes(3);
    expect(api.fetchRuntimeDataModelMetadata).toHaveBeenCalledTimes(3);
  });
});
