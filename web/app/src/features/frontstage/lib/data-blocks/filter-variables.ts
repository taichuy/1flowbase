import { i18nText } from '../../../../shared/i18n/text';
import type { BlockContext } from '@1flowbase/page-protocol';
import type {
  DataBlockField,
  DataFilterCondition,
  DataFilterGroup
} from './types';
import { isDataFilterGroup } from './filters';

export interface DataFilterVariable {
  value: string;
  label: string;
  resolved: unknown;
}

/** Only values already exposed by BlockContext; no expression evaluation or extra requests. */
export function dataFilterVariables(ctx: BlockContext): DataFilterVariable[] {
  const result: DataFilterVariable[] = [];
  const labels: Record<string, string> = {
    currentUser: i18nText('frontstage', 'data_blocks.filter.current_user'),
    workspace: i18nText('frontstage', 'data_blocks.filter.workspace'),
    application: i18nText('frontstage', 'data_blocks.filter.application'),
    page: i18nText('frontstage', 'data_blocks.filter.page'),
    inputs: i18nText('frontstage', 'data_blocks.filter.inputs'),
    params: i18nText('frontstage', 'data_blocks.filter.params')
  };
  for (const [root, value] of Object.entries({
    currentUser: ctx.currentUser && {
      id: ctx.currentUser.id,
      displayName: ctx.currentUser.displayName
    },
    workspace: ctx.workspace,
    application: ctx.application,
    page: ctx.page,
    inputs: ctx.inputs,
    params: ctx.params
  })) {
    if (!value) continue;
    for (const [key, resolved] of Object.entries(value)) {
      if (
        resolved === undefined ||
        (typeof resolved === 'object' &&
          resolved !== null &&
          !Array.isArray(resolved))
      )
        continue;
      result.push({
        value: `${root}.${key}`,
        label: `${labels[root]} / ${key === 'id' ? i18nText('frontstage', 'data_blocks.filter.id') : key === 'displayName' ? i18nText('frontstage', 'data_blocks.filter.display_name') : key}`,
        resolved
      });
    }
  }
  return result;
}

export function validDataFilter(
  group: DataFilterGroup,
  fields: readonly DataBlockField[],
  variables?: readonly DataFilterVariable[]
): boolean {
  return group.conditions.every((condition) => {
    if (isDataFilterGroup(condition))
      return (
        condition.conditions.length > 0 &&
        validDataFilter(condition, fields, variables)
      );
    const field = fields.find((field) => field.code === condition.field_code);
    if (!field) return false;
    const value =
      condition.value_source === 'variable'
        ? variables?.find((variable) => variable.value === condition.value)
            ?.resolved
        : condition.value;
    if (condition.operator === '$in' && !Array.isArray(value)) return false;
    const values = Array.isArray(value) ? value : [value];
    if (
      !values.every((item) => {
        if (item === null) return true;
        if (field.field_kind === 'number')
          return typeof item === 'number' && Number.isFinite(item);
        if (field.field_kind === 'boolean') return typeof item === 'boolean';
        if (field.field_kind === 'datetime')
          return typeof item === 'string' && !Number.isNaN(Date.parse(item));
        return typeof item !== 'number' || Number.isFinite(item);
      })
    )
      return false;
    return (
      value !== undefined &&
      value !== '' &&
      (Array.isArray(value)
        ? value.length > 0
        : !(typeof value === 'number' && !Number.isFinite(value)))
    );
  });
}

export function restrictDataFilter(
  group: DataFilterGroup,
  codes: readonly string[]
): DataFilterGroup {
  return {
    ...group,
    conditions: group.conditions.flatMap<DataFilterCondition | DataFilterGroup>(
      (condition) => {
        if (!isDataFilterGroup(condition))
          return codes.includes(condition.field_code) ? [condition] : [];
        const nested = restrictDataFilter(condition, codes);
        return nested.conditions.length ? [nested] : [];
      }
    )
  };
}
