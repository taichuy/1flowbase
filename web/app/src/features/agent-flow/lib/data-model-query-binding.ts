import type {
  DataModelQueryBindingValue,
  DataModelQueryFilter,
  DataModelQueryOperator,
  DataModelQuerySort,
  DataModelQueryValue,
  FlowBinding,
  FlowNodeDocument
} from '@1flowbase/flow-schema';
import { getDataModelActionForNodeType } from './node-definitions/nodes/data-model';

export const DATA_MODEL_QUERY_OPERATORS: DataModelQueryOperator[] = [
  'eq',
  'ne',
  'gt',
  'gte',
  'lt',
  'lte'
];

export const DATA_MODEL_QUERY_DEFAULT_VALUE: DataModelQueryBindingValue = {
  filters: [],
  sorts: [],
  expand_relations: [],
  page: { kind: 'constant', value: 1 },
  page_size: { kind: 'constant', value: 20 }
};

const DATA_MODEL_QUERY_OPERATOR_SET = new Set<string>(
  DATA_MODEL_QUERY_OPERATORS
);

const ACTIVE_BINDINGS: Record<string, string[]> = {
  list: ['query'],
  get: ['record_id'],
  create: ['payload'],
  update: ['record_id', 'payload'],
  delete: ['record_id']
};

export function getActiveNodeBindings(node: FlowNodeDocument) {
  const action = getDataModelActionForNodeType(node.type);

  if (!action) {
    return Object.entries(node.bindings);
  }

  const activeKeys = new Set(ACTIVE_BINDINGS[action]);

  return Object.entries(node.bindings).filter(([key]) => activeKeys.has(key));
}

function createDefaultDataModelQueryBindingValue(): DataModelQueryBindingValue {
  return {
    filters: [],
    sorts: [],
    expand_relations: [],
    page: { kind: 'constant', value: 1 },
    page_size: { kind: 'constant', value: 20 }
  };
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function normalizeSelectorPath(value: unknown) {
  return Array.isArray(value)
    ? value.filter((segment): segment is string => typeof segment === 'string')
    : [];
}

function normalizeDataModelQueryFilter(
  value: unknown
): DataModelQueryFilter | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const fieldCode =
    typeof value.field_code === 'string' ? value.field_code.trim() : '';
  const operator =
    typeof value.operator === 'string' &&
    DATA_MODEL_QUERY_OPERATOR_SET.has(value.operator)
      ? (value.operator as DataModelQueryOperator)
      : null;

  if (!fieldCode || !operator) {
    return null;
  }

  return {
    field_code: fieldCode,
    operator,
    value: normalizeDataModelQueryValue(value.value, '')
  };
}

function normalizeDataModelQuerySort(
  value: unknown
): DataModelQuerySort | null {
  if (!isObjectRecord(value)) {
    return null;
  }

  const fieldCode =
    typeof value.field_code === 'string' ? value.field_code.trim() : '';
  const direction =
    value.direction === 'desc'
      ? 'desc'
      : value.direction === 'asc'
        ? 'asc'
        : null;

  if (!fieldCode || !direction) {
    return null;
  }

  return {
    field_code: fieldCode,
    direction
  };
}

export function normalizeDataModelQueryBindingValue(
  value: unknown
): DataModelQueryBindingValue {
  if (!isObjectRecord(value)) {
    return createDefaultDataModelQueryBindingValue();
  }

  return {
    filters: Array.isArray(value.filters)
      ? value.filters.flatMap((entry) => {
          const filter = normalizeDataModelQueryFilter(entry);

          return filter ? [filter] : [];
        })
      : [],
    sorts: Array.isArray(value.sorts)
      ? value.sorts.flatMap((entry) => {
          const sort = normalizeDataModelQuerySort(entry);

          return sort ? [sort] : [];
        })
      : [],
    expand_relations: Array.isArray(value.expand_relations)
      ? value.expand_relations.filter(
          (entry): entry is string => typeof entry === 'string'
        )
      : [],
    page: normalizeDataModelQueryValue(value.page, 1),
    page_size: normalizeDataModelQueryValue(value.page_size, 20)
  };
}

export function normalizeDataModelQueryValue(
  value: unknown,
  fallback: unknown
): DataModelQueryValue {
  if (!isObjectRecord(value)) {
    return { kind: 'constant', value: fallback };
  }

  if (value.kind === 'selector') {
    return {
      kind: 'selector',
      selector: normalizeSelectorPath(value.selector)
    };
  }

  if (value.kind === 'constant') {
    return { kind: 'constant', value: value.value };
  }

  return { kind: 'constant', value: fallback };
}

export function extractDataModelQuerySelectors(
  value: unknown
) {
  const query = normalizeDataModelQueryBindingValue(value);
  const selectors = query.filters.flatMap((filter) =>
    filter.value.kind === 'selector' ? [filter.value.selector] : []
  );

  if (query.page.kind === 'selector') {
    selectors.push(query.page.selector);
  }

  if (query.page_size.kind === 'selector') {
    selectors.push(query.page_size.selector);
  }

  return selectors;
}

export function remapDataModelQueryBinding(
  binding: FlowBinding,
  remapSelector: (selector: string[]) => string[]
): FlowBinding {
  if (binding.kind !== 'data_model_query') {
    return binding;
  }

  const value = normalizeDataModelQueryBindingValue(binding.value);
  const remapValue = (value: DataModelQueryValue): DataModelQueryValue =>
    value.kind === 'selector'
      ? { ...value, selector: remapSelector(value.selector) }
      : value;

  return {
    ...binding,
    value: {
      ...value,
      filters: value.filters.map((filter: DataModelQueryFilter) => ({
        ...filter,
        value: remapValue(filter.value)
      })),
      page: remapValue(value.page),
      page_size: remapValue(value.page_size)
    }
  };
}
