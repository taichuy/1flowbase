import {
  matchesDataFilter,
  relationRecordId
} from '../../../lib/data-blocks/filters';
import type {
  DataBlockField,
  DataFormItem,
  FormBlockConfig
} from '../../../lib/data-blocks/types';

export function formRecordId(
  inputs: Readonly<Record<string, unknown>>
): string | null {
  const value = inputs.record_id;
  return typeof value === 'string' && value.trim()
    ? value
    : typeof value === 'number'
      ? String(value)
      : null;
}

export function recordIdentity(
  config: FormBlockConfig,
  inputs: Readonly<Record<string, unknown>>
): string {
  return `${config.binding.model_code}:${config.mode}:${config.mode === 'create' ? (relationRecordId(config.relation, inputs) ?? '') : (formRecordId(inputs) ?? '')}`;
}

export function defaultFormValues(
  items: DataFormItem[]
): Record<string, unknown> {
  return Object.fromEntries(
    items
      .filter(
        (item) =>
          item.kind === 'field' &&
          item.field_code &&
          item.default_value !== undefined
      )
      .map((item) => [item.field_code!, item.default_value])
  );
}

export function fieldState(
  field: DataBlockField,
  item: DataFormItem,
  config: FormBlockConfig,
  values: Readonly<Record<string, unknown>>
) {
  let visible = true;
  let required = Boolean(item.required);
  let readOnly = Boolean(item.read_only);
  for (const rule of config.rules) {
    if (
      rule.target_field !== field.code ||
      !matchesDataFilter(rule.condition, values)
    )
      continue;
    if (rule.effect === 'show') visible = true;
    if (rule.effect === 'hide') visible = false;
    if (rule.effect === 'required') required = true;
    if (rule.effect === 'optional') required = false;
    if (rule.effect === 'readonly') readOnly = true;
    if (rule.effect === 'editable') readOnly = false;
  }
  return {
    visible,
    required: required || field.api_required || field.is_required,
    readOnly:
      readOnly ||
      !field.is_writable ||
      field.field_kind === 'one_to_many' ||
      config.mode === 'view'
  };
}

export const isEmptyFormValue = (value: unknown) =>
  value == null || value === '' || (Array.isArray(value) && value.length === 0);
const isRecordId = (value: unknown) =>
  (typeof value === 'string' && value.trim().length > 0) ||
  typeof value === 'number';

export function formWriteValues(
  config: FormBlockConfig,
  fields: DataBlockField[],
  draft: Readonly<Record<string, unknown>>,
  inputs: Readonly<Record<string, unknown>>
) {
  const values: Record<string, unknown> = {};
  for (const item of config.items) {
    const field = fields.find(
      (candidate) => candidate.code === item.field_code
    );
    if (!field || item.kind !== 'field') continue;
    const state = fieldState(field, item, config, draft);
    if (!state.visible || state.readOnly || draft[field.code] === undefined)
      continue;
    const value = draft[field.code];
    if (
      field.field_kind === 'many_to_one' &&
      value != null &&
      !isRecordId(value)
    )
      throw new Error('invalid_relation');
    if (
      field.field_kind === 'many_to_many' &&
      value != null &&
      (!Array.isArray(value) || !value.every(isRecordId))
    )
      throw new Error('invalid_relation');
    values[field.code] = value;
  }
  if (config.mode === 'create' && config.relation) {
    const parent = relationRecordId(config.relation, inputs);
    if (!parent) throw new Error('missing_parent');
    const relationField = fields.find(
      (field) => field.code === config.relation!.field_code
    );
    if (
      !relationField?.is_writable ||
      relationField.field_kind === 'one_to_many'
    )
      throw new Error('readonly_parent');
    values[config.relation.field_code] = parent;
  }
  return values;
}

export function moveFormItem(
  items: DataFormItem[],
  activeId: string,
  targetId: string,
  placement: 'before' | 'after' | 'beside'
): DataFormItem[] {
  if (activeId === targetId) return items;
  const active = items.find((item) => item.id === activeId);
  const target = items.find((item) => item.id === targetId);
  if (!active || !target) return items;
  const next = items
    .filter((item) => item.id !== activeId)
    .map((item) => ({ ...item }));
  const targetIndex = next.findIndex((item) => item.id === targetId);
  const rowIndices = next.flatMap((item, index) =>
    item.row === target.row ? [index] : []
  );
  const index =
    placement === 'before'
      ? rowIndices[0]
      : placement === 'after'
        ? rowIndices[rowIndices.length - 1] + 1
        : targetIndex + 1;
  let newRow = `row-${active.id}`;
  while (next.some((item) => item.row === newRow)) newRow += '-next';
  next.splice(index, 0, {
    ...active,
    row: placement === 'beside' ? target.row : newRow,
    span: placement === 'beside' ? 12 : 24
  });
  if (placement === 'beside') {
    const row = next.filter((item) => item.row === target.row);
    for (const item of row)
      item.span = Math.max(1, Math.floor(24 / row.length));
  }
  return next;
}
