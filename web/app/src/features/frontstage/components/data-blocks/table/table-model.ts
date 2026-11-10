import type { DataBlockHost } from '../../../lib/data-blocks/host-context';
import {
  dataFilterToRequest,
  intersectDataFilters,
  relationRecordId
} from '../../../lib/data-blocks/filters';
import type {
  DataBlockAction,
  DataBlockMetadata,
  DataBlockField,
  DataFilterGroup,
  TableBlockConfig
} from '../../../lib/data-blocks/types';

export function tableRecordId(
  record: Record<string, unknown>,
  field: string
): string | null {
  const value = record[field];
  return typeof value === 'string' && value.length > 0
    ? value
    : typeof value === 'number' && Number.isFinite(value)
      ? String(value)
      : null;
}

export function tableQueryScope(
  config: TableBlockConfig,
  userFilter: DataFilterGroup,
  inputs: Readonly<Record<string, unknown>>,
  variables: Readonly<Record<string, unknown>> = {}
) {
  const parentId = relationRecordId(config.relation, inputs);
  const linked = inputs.linked_value;
  const linkedIds = Array.isArray(linked)
    ? linked.filter(
        (value): value is string =>
          typeof value === 'string' && value.length > 0
      )
    : [];
  const linkedId =
    typeof linked === 'string' && linked.length
      ? linked
      : typeof linked === 'number'
        ? String(linked)
        : null;
  const linkageReady =
    !config.linkage ||
    (config.linkage.source_output === 'selected_record_ids'
      ? linkedIds.length > 0
      : linkedId !== null);
  return {
    enabled: (!config.relation || parentId !== null) && linkageReady,
    filter: intersectDataFilters(
      dataFilterToRequest(config.filter, variables),
      dataFilterToRequest(userFilter, variables),
      config.relation && parentId !== null
        ? { [config.relation.field_code]: { $eq: parentId } }
        : null,
      config.linkage
        ? {
            [config.linkage.field_code]:
              config.linkage.source_output === 'selected_record_ids'
                ? { $in: linkedIds }
                : { $eq: linkedId }
          }
        : null
    )
  };
}

export function tableActionAllowed(
  action: DataBlockAction,
  capabilities: DataBlockMetadata['record_capabilities'] | undefined
): boolean {
  if (!action.enabled || !capabilities) return false;
  switch (action.kind) {
    case 'create':
      return capabilities.can_create;
    case 'edit':
      return capabilities.can_update && capabilities.can_get;
    case 'view':
    case 'open':
      return capabilities.can_get;
    case 'delete':
      return capabilities.can_delete;
    default:
      return capabilities.can_list;
  }
}

export function tableActionInputs(
  config: TableBlockConfig,
  blockId: string,
  inputs: Readonly<Record<string, unknown>>,
  recordId?: string
): Record<string, string> {
  const parentId = relationRecordId(config.relation, inputs);
  return {
    ...(config.relation && parentId !== null
      ? {
          [config.relation.parent_record_input]: parentId,
          parent_record_id: parentId,
          parent_model_code: config.relation.parent_model_code
        }
      : {}),
    source_block_id: blockId,
    source_model_code: config.binding.model_code,
    ...(recordId !== undefined ? { record_id: recordId } : {})
  };
}

/** Deletes are independent backend operations. Keep failed IDs selected for retry. */
export async function deleteTableRecords(
  ids: readonly string[],
  remove: (id: string) => Promise<unknown>
) {
  const deleted: string[] = [];
  const failed: string[] = [];
  for (const id of new Set(ids)) {
    try {
      await remove(id);
      deleted.push(id);
    } catch {
      failed.push(id);
    }
  }
  return { deleted, failed };
}

/** A relation-backed linkage must also scope child form writes to one parent. */
export function tableActionParentContext(
  config: TableBlockConfig,
  fields: DataBlockField[],
  inputs: Readonly<Record<string, unknown>>
) {
  if (config.relation)
    return {
      relation: config.relation,
      inputs,
      canCreate: relationRecordId(config.relation, inputs) !== null
    };
  const field = config.linkage
    ? fields.find((item) => item.code === config.linkage!.field_code)
    : undefined;
  if (
    !field ||
    field.field_kind !== 'many_to_one' ||
    !field.relation_target_model_code
  )
    return { relation: undefined, inputs, canCreate: true };
  const linked = inputs.linked_value;
  const value = Array.isArray(linked)
    ? linked.length === 1
      ? linked[0]
      : null
    : linked;
  const parentRecordId =
    typeof value === 'string' && value.length > 0
      ? value
      : typeof value === 'number'
        ? String(value)
        : '';
  return {
    relation: {
      field_code: field.code,
      parent_model_code: field.relation_target_model_code,
      parent_record_input: 'parent_record_id'
    },
    inputs: { ...inputs, parent_record_id: parentRecordId },
    canCreate: parentRecordId.length > 0
  };
}

export async function prepareTableAction(
  host: DataBlockHost,
  config: TableBlockConfig,
  action: DataBlockAction,
  fields: DataBlockField[] = []
) {
  if (
    !action.enabled ||
    !['create', 'view', 'edit', 'open'].includes(action.kind)
  )
    return action;
  return host.prepareAction(
    action,
    config.binding,
    action.kind === 'create'
      ? 'create'
      : action.kind === 'edit'
        ? 'edit'
        : 'view',
    tableActionParentContext(config, fields, {}).relation
  );
}

export async function persistTableConfig(
  host: DataBlockHost,
  config: TableBlockConfig,
  committed: (config: TableBlockConfig) => void
) {
  await host.saveConfig(config);
  committed(config);
}
