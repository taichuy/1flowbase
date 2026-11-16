import { useEffect, useMemo, useReducer } from 'react';
import type { BlockContext } from '@1flowbase/page-protocol';
import {
  dataBlockRecordsKey,
  useDataBlockMetadata,
  useDataBlockRecords
} from '../../../../api/data-blocks';
import {
  dataFilterVariables,
  validDataFilter
} from '../../../../lib/data-blocks/filter-variables';
import type {
  DataBlockField,
  DataFilterGroup,
  TableBlockConfig
} from '../../../../lib/data-blocks/types';
import { tableQueryScope, tableRecordId } from '../table-model';
import {
  createTableSession,
  reconcileTableSession,
  tableSessionContextKey,
  tableSessionReducer,
  tableSessionRequestKey,
  type TableSessionContext
} from './table-session';

function filterFields(group: DataFilterGroup): string[] {
  return group.conditions.flatMap((condition) =>
    'conditions' in condition ? filterFields(condition) : [condition.field_code]
  );
}

export function useTableSession(
  ctx: BlockContext,
  config: TableBlockConfig,
  metadata: ReturnType<typeof useDataBlockMetadata>
) {
  const fields = metadata.data?.fields ?? [];
  const fieldsByCode = new Map<string, DataBlockField>();
  for (const field of fields) {
    if (!fieldsByCode.has(field.code)) fieldsByCode.set(field.code, field);
  }
  const variables = dataFilterVariables(ctx);
  const idField = metadata.data?.record_id_field ?? '';
  const context: TableSessionContext = {
    ownerKey: JSON.stringify([
      ...dataBlockRecordsKey(ctx, config.binding.model_code),
      config.binding.model_id,
      config.binding.data_source_id
    ]),
    config,
    inputs: ctx.inputs,
    variables: Object.fromEntries(variables.map((variable) => [variable.value, variable.resolved])),
    idField
  };
  const [stored, dispatch] = useReducer(tableSessionReducer, context, createTableSession);
  const state = reconcileTableSession(stored, context);
  // Persist the semantic transition in this render, before effects or query subscription.
  if (state !== stored) dispatch({ type: 'context', context });
  const scope = tableQueryScope(config, state.filter, ctx.inputs, context.variables);
  const capabilities = metadata.data?.record_capabilities;
  const invalidFields = metadata.data
    ? config.columns.filter((column) => !fieldsByCode.has(column.field_code)) : [];
  const invalidQuery = Boolean(metadata.data && (
    !validDataFilter(config.filter, fields, variables) ||
    !validDataFilter(state.filter, fields, variables) ||
    [
      ...filterFields(config.filter),
      ...filterFields(state.filter),
      ...(state.sort ? [state.sort.field] : []),
      ...(config.relation ? [config.relation.field_code] : []),
      ...(config.linkage ? [config.linkage.field_code] : [])
    ].some((code) => !fieldsByCode.has(code))
  ));
  const records = useDataBlockRecords(ctx, config.binding, {
    page: state.page,
    page_size: state.pageSize,
    filter: scope.filter,
    ...(state.sort ? { sort: state.sort } : {})
  }, Boolean(capabilities?.can_list && scope.enabled && !invalidQuery));
  const data = useMemo(() => records.data?.items ?? [], [records.data?.items]);
  const canAddressRecords = Boolean(idField);
  const missingRecordId = canAddressRecords && data.some((record) => tableRecordId(record, idField) === null);
  const displayKeys = useMemo(() => new Map(data.map((record, index) => [record, `display-only-${index}`])), [data]);
  const selected = state.selected;
  // This is external BlockContext synchronization, not a chain of internal state effects.
  useEffect(() => {
    void ctx.outputs.publish({ record_id: selected[0] ?? '', selected_record_ids: selected });
  }, [ctx.outputs, selected]);

  return {
    ...state,
    contextKey: tableSessionContextKey(state),
    requestKey: tableSessionRequestKey(state),
    fields,
    fieldsByCode,
    variables,
    scope,
    capabilities,
    invalidFields,
    invalidQuery,
    records,
    data,
    canAddressRecords,
    missingRecordId,
    displayKeys,
    paginate(page: number, pageSize: number) { dispatch({ type: 'paginate', page, pageSize }); },
    sortBy(sort: TableBlockConfig['sort']) { dispatch({ type: 'sort', sort }); },
    applyFilter(filter: DataFilterGroup) { dispatch({ type: 'filter', filter, context }); },
    select(selected: string[]) { dispatch({ type: 'select', selected }); },
    completeDelete(event: Omit<Extract<Parameters<typeof tableSessionReducer>[1], { type: 'delete-completed' }>, 'type'>) {
      dispatch({ type: 'delete-completed', ...event });
    }
  };
}
