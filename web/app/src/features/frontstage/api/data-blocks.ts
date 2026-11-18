import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { BlockContext } from '@1flowbase/page-protocol';
import {
  fetchRuntimeDataModelMetadata,
  type ConsoleRuntimeModelRecord,
  type ConsoleRuntimeRecordPreview,
  type FetchConsoleRuntimeModelRecordsInput
} from '@1flowbase/api-client';
import type { DataBlockBinding } from '../lib/data-blocks/types';
import { getFrontstageApiBaseUrl } from './page-tree';

export function dataBlockRecordsKey(ctx: BlockContext, modelCode?: string) {
  const key = [
    'frontstage-data-records',
    ctx.workspace.id,
    ctx.currentUser?.id ?? 'anonymous',
    {
      isRoot: Boolean(ctx.currentUser?.isRoot),
      permissions: [...(ctx.currentUser?.permissions ?? [])].sort()
    }
  ];
  return modelCode ? [...key, modelCode] : key;
}

export function dataBlockMetadataKey(ctx: BlockContext, modelCode: string) {
  return [
    'frontstage-data-metadata',
    ...dataBlockRecordsKey(ctx).slice(1),
    modelCode
  ];
}

export function useDataBlockMetadata(
  ctx: BlockContext,
  binding: DataBlockBinding | undefined
) {
  return useQuery({
    queryKey: dataBlockMetadataKey(ctx, binding?.model_code ?? ''),
    enabled: Boolean(binding?.model_code),
    refetchOnWindowFocus: false,
    queryFn: () =>
      fetchRuntimeDataModelMetadata(
        binding!.model_code,
        getFrontstageApiBaseUrl()
      ),
    retry: false
  });
}

function modelPath(modelCode: string, operation: string) {
  return `/api/runtime/models/${encodeURIComponent(modelCode)}/${operation}`;
}

function listQuery(
  input: FetchConsoleRuntimeModelRecordsInput
): Record<string, unknown> {
  return {
    page: input.page ?? 1,
    page_size: input.page_size ?? 20,
    ...(input.filter ? { filter: JSON.stringify(input.filter) } : {}),
    ...(input.sort
      ? {
          sort:
            typeof input.sort === 'string'
              ? input.sort
              : `${input.sort.field}:${input.sort.direction}`
        }
      : {}),
    ...(input.expand
      ? {
          expand: Array.isArray(input.expand)
            ? input.expand.join(',')
            : input.expand
        }
      : {})
  };
}

export function useDataBlockRecords(
  ctx: BlockContext,
  binding: DataBlockBinding | undefined,
  input: FetchConsoleRuntimeModelRecordsInput,
  enabled = true
) {
  return useQuery({
    queryKey: [...dataBlockRecordsKey(ctx, binding?.model_code), 'list', input],
    enabled: Boolean(binding?.model_code) && enabled,
    refetchOnWindowFocus: false,
    queryFn: () =>
      ctx.api.get<ConsoleRuntimeRecordPreview>(
        modelPath(binding!.model_code, 'list'),
        { query: listQuery(input) }
      ),
    retry: false
  });
}

export function useDataBlockRecord(
  ctx: BlockContext,
  binding: DataBlockBinding | undefined,
  recordId: string | null,
  enabled = true
) {
  return useQuery({
    queryKey: [
      ...dataBlockRecordsKey(ctx, binding?.model_code),
      'record',
      recordId
    ],
    enabled: Boolean(binding?.model_code && recordId) && enabled,
    refetchOnWindowFocus: false,
    queryFn: () =>
      ctx.api.get<ConsoleRuntimeModelRecord>(
        modelPath(binding!.model_code, 'get/{id}'),
        { path: { id: recordId } }
      ),
    retry: false
  });
}

export function useDataBlockWrites(
  ctx: BlockContext,
  binding: DataBlockBinding,
  sourceModelCode?: string
) {
  const queryClient = useQueryClient();
  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({
        queryKey: dataBlockRecordsKey(ctx, binding.model_code)
      }),
      ...(sourceModelCode && sourceModelCode !== binding.model_code
        ? [
            queryClient.invalidateQueries({
              queryKey: dataBlockRecordsKey(ctx, sourceModelCode)
            })
          ]
        : [])
    ]);
  };
  const create = useMutation({
    mutationFn: (values: ConsoleRuntimeModelRecord) =>
      ctx.api.post<ConsoleRuntimeModelRecord>(
        modelPath(binding.model_code, 'create'),
        { body: values }
      ),
    onSuccess: refresh
  });
  const update = useMutation({
    mutationFn: ({
      record_id,
      values
    }: {
      record_id: string;
      values: ConsoleRuntimeModelRecord;
    }) =>
      ctx.api.patch<ConsoleRuntimeModelRecord>(
        modelPath(binding.model_code, 'update/{id}'),
        { path: { id: record_id }, body: values }
      ),
    onSuccess: refresh
  });
  const remove = useMutation({
    mutationFn: (recordId: string) =>
      ctx.api.delete<void>(modelPath(binding.model_code, 'delete/{id}'), {
        path: { id: recordId }
      }),
    onSuccess: refresh
  });
  return { create, update, remove, refresh };
}
