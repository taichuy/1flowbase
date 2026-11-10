import type {
  DataFilterCondition,
  DataFilterGroup,
  DataBlockRelation
} from './types';

export function isDataFilterGroup(
  value: DataFilterCondition | DataFilterGroup
): value is DataFilterGroup {
  return 'conditions' in value;
}

/** Local editor state is translated to the existing backend filter contract. */
export function dataFilterToRequest(
  group: DataFilterGroup,
  variables: Readonly<Record<string, unknown>> = {}
): Record<string, unknown> {
  const conditions = group.conditions.map((item) =>
    isDataFilterGroup(item)
      ? dataFilterToRequest(item, variables)
      : {
          [item.field_code]: {
            [item.operator]:
              (item.value_source === 'variable'
                ? variables[String(item.value)]
                : item.value) ?? null
          }
        }
  );
  return { [group.conjunction === 'and' ? '$and' : '$or']: conditions };
}

export function intersectDataFilters(
  ...filters: Array<Record<string, unknown> | null | undefined>
): Record<string, unknown> {
  return {
    $and: filters.filter((filter): filter is Record<string, unknown> =>
      Boolean(filter)
    )
  };
}

export function relationRecordId(
  relation: DataBlockRelation | undefined,
  inputs: Readonly<Record<string, unknown>>
): string | null {
  if (!relation) return null;
  const value = inputs[relation.parent_record_input];
  return typeof value === 'string' && value.length > 0
    ? value
    : typeof value === 'number'
      ? String(value)
      : null;
}

export function matchesDataFilter(
  group: DataFilterGroup,
  values: Readonly<Record<string, unknown>>
): boolean {
  const match = (item: DataFilterCondition | DataFilterGroup): boolean => {
    if (isDataFilterGroup(item)) return matchesDataFilter(item, values);
    if (!item.field_code || item.value === undefined) return false;
    const actual = values[item.field_code];
    const expected = item.value;
    switch (item.operator) {
      case '$eq':
        return actual === expected || (actual == null && expected == null);
      case '$ne':
        return !(actual === expected || (actual == null && expected == null));
      case '$in':
        return Array.isArray(expected) && expected.includes(actual);
      case '$includes':
        return (
          actual != null &&
          String(actual)
            .toLowerCase()
            .includes(String(expected ?? '').toLowerCase())
        );
      case '$notIncludes':
        return (
          actual == null ||
          !String(actual)
            .toLowerCase()
            .includes(String(expected ?? '').toLowerCase())
        );
      case '$gt':
        return (
          actual != null &&
          expected != null &&
          (typeof actual === 'number' && typeof expected === 'number'
            ? actual > expected
            : String(actual) > String(expected))
        );
      case '$gte':
        return (
          actual != null &&
          expected != null &&
          (typeof actual === 'number' && typeof expected === 'number'
            ? actual >= expected
            : String(actual) >= String(expected))
        );
      case '$lt':
        return (
          actual != null &&
          expected != null &&
          (typeof actual === 'number' && typeof expected === 'number'
            ? actual < expected
            : String(actual) < String(expected))
        );
      case '$lte':
        return (
          actual != null &&
          expected != null &&
          (typeof actual === 'number' && typeof expected === 'number'
            ? actual <= expected
            : String(actual) <= String(expected))
        );
    }
  };
  return group.conjunction === 'and'
    ? group.conditions.every(match)
    : group.conditions.some(match);
}
