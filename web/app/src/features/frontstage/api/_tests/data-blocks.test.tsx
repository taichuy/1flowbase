import type { ReactNode } from 'react';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { BlockContext } from '@1flowbase/page-protocol';
import {
  dataBlockMetadataKey,
  dataBlockRecordsKey,
  useDataBlockMetadata,
  useDataBlockRecord,
  useDataBlockRecords,
  useDataBlockWrites
} from '../data-blocks';

const mocks = vi.hoisted(() => ({ metadata: vi.fn() }));
vi.mock('@1flowbase/api-client', () => ({
  fetchRuntimeDataModelMetadata: mocks.metadata
}));
vi.mock('../page-tree', () => ({
  getFrontstageApiBaseUrl: () => '/workspace-api'
}));

const binding = {
  model_id: 'orders-model',
  model_code: 'orders',
  data_source_id: null
};
const clients: QueryClient[] = [];
function createContext(
  permissions: readonly string[] = ['records.list', 'records.get'],
  isRoot = false,
  userId = 'user-A',
  workspaceId = 'workspace-A'
) {
  return {
    workspace: { id: workspaceId },
    currentUser: { id: userId, permissions, isRoot },
    api: {
      get: vi.fn(),
      post: vi.fn(),
      patch: vi.fn(),
      put: vi.fn(),
      delete: vi.fn()
    }
  } as unknown as BlockContext;
}
function harness() {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: Infinity, staleTime: Infinity },
      mutations: { retry: false }
    }
  });
  clients.push(client);
  return {
    client,
    wrapper: ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    )
  };
}
function seedRecordList(
  client: QueryClient,
  ctx: BlockContext,
  modelCode: string
) {
  const key = [...dataBlockRecordsKey(ctx, modelCode), 'list', { page: 1 }];
  const data = {
    items: [{ order_no: 'existing', title: 'Original' }],
    total: 1
  };
  client.setQueryData(key, data);
  return { key, data };
}

beforeEach(() => vi.resetAllMocks());
afterEach(() => {
  cleanup();
  for (const client of clients.splice(0)) client.clear();
});

describe('data block query transport', () => {
  it('serializes server paging, filter, sort and expansion without changing their backend field names', async () => {
    const ctx = createContext();
    const response = { items: [{ order_no: 'A', amount: 35 }], total: 91 };
    vi.mocked(ctx.api.get).mockResolvedValue(response);
    const filter = {
      $and: [{ amount: { $gte: 30 } }, { customer_id: { $eq: 'customer-A' } }]
    };
    const { wrapper } = harness();
    const { result } = renderHook(
      () =>
        useDataBlockRecords(ctx, binding, {
          page: 3,
          page_size: 10,
          filter,
          sort: { field: 'amount', direction: 'desc' },
          expand: ['customer_id', 'owner_id']
        }),
      { wrapper }
    );
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(ctx.api.get).toHaveBeenCalledWith(
      '/api/runtime/models/orders/list',
      {
        query: {
          page: 3,
          page_size: 10,
          filter: JSON.stringify(filter),
          sort: 'amount:desc',
          expand: 'customer_id,owner_id'
        }
      }
    );
    expect(result.current.data).toEqual(response);
  });

  it('uses a canonical get/{id} path with a separate unmodified record identifier', async () => {
    const ctx = createContext();
    const recordId = 'order/中文 ?#';
    const record = { order_no: recordId, title: 'Order A' };
    vi.mocked(ctx.api.get).mockResolvedValue(record);
    const { wrapper } = harness();
    const { result } = renderHook(
      () =>
        useDataBlockRecord(
          ctx,
          { ...binding, model_code: 'sales/orders' },
          recordId
        ),
      { wrapper }
    );
    await waitFor(() => expect(result.current.data).toEqual(record));
    expect(ctx.api.get).toHaveBeenCalledWith(
      '/api/runtime/models/sales%2Forders/get/{id}',
      { path: { id: recordId } }
    );
  });

  it('does not request records without a binding, record ID or enabled scope', async () => {
    const ctx = createContext();
    const { wrapper } = harness();
    const { result, rerender } = renderHook(
      ({ enabled }: { enabled: boolean }) => ({
        missingBinding: useDataBlockRecords(ctx, undefined, {}),
        missingId: useDataBlockRecord(ctx, binding, null),
        scopedList: useDataBlockRecords(ctx, binding, {}, enabled),
        scopedRecord: useDataBlockRecord(ctx, binding, 'A', enabled)
      }),
      { wrapper, initialProps: { enabled: false } }
    );
    expect(ctx.api.get).not.toHaveBeenCalled();
    expect(result.current.scopedList.fetchStatus).toBe('idle');
    vi.mocked(ctx.api.get).mockImplementation(
      async (path) =>
        (path.endsWith('/list')
          ? { items: [], total: 0 }
          : { order_no: 'A' }) as never
    );
    rerender({ enabled: true });
    await waitFor(() =>
      expect(result.current.scopedRecord.isSuccess).toBe(true)
    );
    expect(ctx.api.get).toHaveBeenCalledTimes(2);
    expect(result.current.missingBinding.fetchStatus).toBe('idle');
    expect(result.current.missingId.fetchStatus).toBe('idle');
  });

  it('returns request failure without fabricating an empty successful list or retrying the denied request', async () => {
    const ctx = createContext();
    const failure = new Error('Forbidden');
    vi.mocked(ctx.api.get).mockRejectedValue(failure);
    const { wrapper } = harness();
    const { result } = renderHook(() => useDataBlockRecords(ctx, binding, {}), {
      wrapper
    });
    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(result.current.error).toBe(failure);
    expect(result.current.data).toBeUndefined();
    expect(ctx.api.get).toHaveBeenCalledTimes(1);
  });

  it('uses the metadata endpoint and suppresses it until a data model is bound', async () => {
    const ctx = createContext();
    const metadata = {
      model_code: 'orders',
      record_id_field: 'order_no',
      fields: [],
      record_capabilities: { can_list: true }
    };
    mocks.metadata.mockResolvedValue(metadata);
    const { wrapper } = harness();
    const { result, rerender } = renderHook(
      ({ bound }: { bound: boolean }) =>
        useDataBlockMetadata(ctx, bound ? binding : undefined),
      { wrapper, initialProps: { bound: false } }
    );
    expect(mocks.metadata).not.toHaveBeenCalled();
    rerender({ bound: true });
    await waitFor(() => expect(result.current.data).toEqual(metadata));
    expect(mocks.metadata).toHaveBeenCalledWith('orders', '/workspace-api');
  });
});

describe('effective permission cache isolation', () => {
  it('scopes both metadata and records to workspace, user, permissions and root status', () => {
    const base = createContext();
    const variants = [
      createContext(['records.list']),
      createContext(undefined, true),
      createContext(undefined, false, 'user-B'),
      createContext(undefined, false, 'user-A', 'workspace-B')
    ];
    for (const ctx of variants) {
      expect(dataBlockRecordsKey(ctx, 'orders')).not.toEqual(
        dataBlockRecordsKey(base, 'orders')
      );
      expect(dataBlockMetadataKey(ctx, 'orders')).not.toEqual(
        dataBlockMetadataKey(base, 'orders')
      );
    }
    expect(dataBlockRecordsKey(base, 'orders')).not.toEqual(
      dataBlockRecordsKey(base, 'customers')
    );
    expect(dataBlockMetadataKey(base, 'orders')).not.toEqual(
      dataBlockMetadataKey(base, 'customers')
    );
  });

  it('normalizes permission order without mutating the host-owned permission array', () => {
    const permissions = Object.freeze([
      'records.update',
      'records.get',
      'records.list'
    ]);
    const reordered = createContext([
      'records.list',
      'records.update',
      'records.get'
    ]);
    const original = createContext(permissions);
    expect(dataBlockRecordsKey(original, 'orders')).toEqual(
      dataBlockRecordsKey(reordered, 'orders')
    );
    expect(dataBlockMetadataKey(original, 'orders')).toEqual(
      dataBlockMetadataKey(reordered, 'orders')
    );
    expect(permissions).toEqual([
      'records.update',
      'records.get',
      'records.list'
    ]);
  });

  it('never exposes cached metadata, list rows or record details after permissions change and the backend denies access', async () => {
    const ctx = createContext();
    vi.mocked(ctx.api.get).mockImplementation(
      async (path) =>
        (path.endsWith('/list')
          ? { items: [{ order_no: 'secret-A' }], total: 1 }
          : { order_no: 'secret-A' }) as never
    );
    mocks.metadata.mockResolvedValue({
      model_code: 'orders',
      record_id_field: 'order_no',
      record_capabilities: { can_list: true, can_get: true }
    });
    const { wrapper } = harness();
    const { result, rerender } = renderHook(
      ({ context }: { context: BlockContext }) => ({
        metadata: useDataBlockMetadata(context, binding),
        list: useDataBlockRecords(context, binding, {}),
        record: useDataBlockRecord(context, binding, 'secret-A')
      }),
      { wrapper, initialProps: { context: ctx } }
    );
    await waitFor(() =>
      expect(
        result.current.metadata.isSuccess &&
          result.current.list.isSuccess &&
          result.current.record.isSuccess
      ).toBe(true)
    );
    const failure = new Error('Permission revoked');
    vi.mocked(ctx.api.get).mockRejectedValue(failure);
    mocks.metadata.mockRejectedValue(failure);
    rerender({
      context: { ...ctx, currentUser: { ...ctx.currentUser!, permissions: [] } }
    });
    expect(result.current.metadata.data).toBeUndefined();
    expect(result.current.list.data).toBeUndefined();
    expect(result.current.record.data).toBeUndefined();
    await waitFor(() =>
      expect(
        result.current.metadata.isError &&
          result.current.list.isError &&
          result.current.record.isError
      ).toBe(true)
    );
    expect(result.current.list.error).toBe(failure);
  });
});

describe('canonical data writes and cache refresh', () => {
  it('creates with backend field names and invalidates target/source records without invalidating unrelated or differently authorized caches', async () => {
    const ctx = createContext();
    const { client, wrapper } = harness();
    const target = seedRecordList(client, ctx, 'orders');
    const source = seedRecordList(client, ctx, 'customers');
    const unrelated = seedRecordList(client, ctx, 'products');
    const otherPermissions = seedRecordList(
      client,
      createContext(['records.list']),
      'orders'
    );
    const otherUser = seedRecordList(
      client,
      createContext(undefined, false, 'user-B'),
      'orders'
    );
    const values = { customer_id: 'customer-A', amount: 20 };
    const created = { order_no: 'order-B', ...values };
    vi.mocked(ctx.api.post).mockResolvedValue(created);
    const { result } = renderHook(
      () => useDataBlockWrites(ctx, binding, 'customers'),
      { wrapper }
    );
    let returned: unknown;
    await act(async () => {
      returned = await result.current.create.mutateAsync(values);
    });
    expect(ctx.api.post).toHaveBeenCalledWith(
      '/api/runtime/models/orders/create',
      { body: values }
    );
    expect(returned).toEqual(created);
    expect(client.getQueryState(target.key)?.isInvalidated).toBe(true);
    expect(client.getQueryState(source.key)?.isInvalidated).toBe(true);
    for (const untouched of [unrelated, otherPermissions, otherUser])
      expect(client.getQueryState(untouched.key)?.isInvalidated).toBe(false);
    expect(client.getQueryData(target.key)).toEqual(target.data);
  });

  it('updates using PATCH and removes using DELETE with canonical {id} placeholders, preserving opaque IDs', async () => {
    const ctx = createContext();
    const { client, wrapper } = harness();
    const target = seedRecordList(client, ctx, 'orders');
    const invalidate = vi.spyOn(client, 'invalidateQueries');
    const recordId = 'order/中文 ?#';
    const values = { amount: 42 };
    vi.mocked(ctx.api.patch).mockResolvedValue({
      order_no: recordId,
      ...values
    });
    vi.mocked(ctx.api.delete).mockResolvedValue(undefined);
    const { result } = renderHook(
      () => useDataBlockWrites(ctx, binding, 'orders'),
      { wrapper }
    );
    await act(async () => {
      await result.current.update.mutateAsync({ record_id: recordId, values });
    });
    expect(ctx.api.patch).toHaveBeenCalledWith(
      '/api/runtime/models/orders/update/{id}',
      { path: { id: recordId }, body: values }
    );
    expect(ctx.api.put).not.toHaveBeenCalled();
    expect(client.getQueryState(target.key)?.isInvalidated).toBe(true);
    expect(invalidate).toHaveBeenCalledTimes(1);
    await act(async () => {
      await result.current.remove.mutateAsync(recordId);
    });
    expect(ctx.api.delete).toHaveBeenCalledWith(
      '/api/runtime/models/orders/delete/{id}',
      { path: { id: recordId } }
    );
    expect(invalidate).toHaveBeenCalledTimes(2);
  });

  it.each(['create', 'update', 'remove'] as const)(
    'propagates %s failure without changing cached rows or publishing successful refresh',
    async (operation) => {
      const ctx = createContext();
      const { client, wrapper } = harness();
      const target = seedRecordList(client, ctx, 'orders');
      const source = seedRecordList(client, ctx, 'customers');
      const invalidate = vi.spyOn(client, 'invalidateQueries');
      const failure = new Error('Write rejected');
      vi.mocked(ctx.api.post).mockRejectedValue(failure);
      vi.mocked(ctx.api.patch).mockRejectedValue(failure);
      vi.mocked(ctx.api.delete).mockRejectedValue(failure);
      const { result } = renderHook(
        () => useDataBlockWrites(ctx, binding, 'customers'),
        { wrapper }
      );
      await act(async () => {
        const pending =
          operation === 'create'
            ? result.current.create.mutateAsync({ amount: 1 })
            : operation === 'update'
              ? result.current.update.mutateAsync({
                  record_id: 'A',
                  values: { amount: 1 }
                })
              : result.current.remove.mutateAsync('A');
        await expect(pending).rejects.toBe(failure);
      });
      await waitFor(() => expect(result.current[operation].isError).toBe(true));
      expect(invalidate).not.toHaveBeenCalled();
      for (const cache of [target, source]) {
        expect(client.getQueryState(cache.key)?.isInvalidated).toBe(false);
        expect(client.getQueryData(cache.key)).toEqual(cache.data);
      }
    }
  );

  it('refreshes active source queries without losing their pagination, filter or sort', async () => {
    const ctx = createContext();
    const first = { items: [{ order_no: 'A', amount: 20 }], total: 1 };
    const refreshed = { items: [{ order_no: 'A', amount: 25 }], total: 1 };
    vi.mocked(ctx.api.get)
      .mockResolvedValueOnce(first)
      .mockResolvedValueOnce(refreshed);
    const { wrapper } = harness();
    const input = {
      page: 2,
      page_size: 20,
      filter: { amount: { $gte: 10 } },
      sort: 'amount:desc'
    };
    const { result } = renderHook(
      () => ({
        records: useDataBlockRecords(ctx, binding, input),
        writes: useDataBlockWrites(ctx, binding)
      }),
      { wrapper }
    );
    await waitFor(() => expect(result.current.records.data).toEqual(first));
    await act(async () => {
      await result.current.writes.refresh();
    });
    await waitFor(() => expect(result.current.records.data).toEqual(refreshed));
    expect(vi.mocked(ctx.api.get).mock.calls[1]).toEqual(
      vi.mocked(ctx.api.get).mock.calls[0]
    );
    expect(ctx.api.get).toHaveBeenLastCalledWith(
      '/api/runtime/models/orders/list',
      {
        query: {
          page: 2,
          page_size: 20,
          filter: JSON.stringify(input.filter),
          sort: 'amount:desc'
        }
      }
    );
  });
});
