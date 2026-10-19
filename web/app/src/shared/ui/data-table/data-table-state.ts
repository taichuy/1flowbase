import { useCallback, useEffect, useMemo, useState } from 'react';
import type { Dispatch, ReactNode, SetStateAction } from 'react';

export const DEFAULT_MIN_COLUMN_WIDTH = 110;

export type DataTableColumn<T extends object> = {
  key: string;
  title: string;
  width: number;
  defaultVisibility?: 'visible' | 'hidden';
  minWidth?: number;
  align?: 'left' | 'center' | 'right';
  sizing?: 'fixed' | 'fill';
  dataIndex?: keyof T;
  render?: (value: unknown, record: T, index: number) => ReactNode;
  ellipsis?: boolean;
  sorter?: boolean;
  sortOrder?: 'ascend' | 'descend' | null;
  mobileRole?: 'title' | 'actions';
};

export type DataTableState = {
  visibleColumnKeys: string[];
  columnWidths: Record<string, number>;
};

export type DataTableConfiguration = DataTableState & {
  setVisibleColumnKeys: Dispatch<SetStateAction<string[]>>;
  setColumnWidths: Dispatch<SetStateAction<Record<string, number>>>;
};

export function getDefaultVisibleKeys<T extends object>(
  columns: Array<DataTableColumn<T>>
) {
  return columns
    .filter((column) => column.defaultVisibility !== 'hidden')
    .map((column) => column.key);
}

export function getDefaultColumnWidths<T extends object>(
  columns: Array<DataTableColumn<T>>
) {
  return columns.reduce<Record<string, number>>((acc, column) => {
    acc[column.key] = column.width;

    return acc;
  }, {});
}

export function getColumnMinWidth<T extends object>(
  column: DataTableColumn<T>
) {
  return column.minWidth ?? DEFAULT_MIN_COLUMN_WIDTH;
}

export function normalizeVisibleKeys<T extends object>(
  columns: Array<DataTableColumn<T>>,
  visibleColumnKeys: string[]
) {
  const defaultVisibleKeys = getDefaultVisibleKeys(columns);
  const normalized = columns
    .map((column) => column.key)
    .filter((key) => visibleColumnKeys.includes(key));

  return normalized.length ? normalized : defaultVisibleKeys;
}

export function normalizeColumnWidths<T extends object>(
  columns: Array<DataTableColumn<T>>,
  columnWidths: Record<string, unknown>
) {
  const normalized = getDefaultColumnWidths(columns);

  columns.forEach((column) => {
    const storedWidth = columnWidths[column.key];

    if (
      typeof storedWidth === 'number' &&
      Number.isFinite(storedWidth) &&
      storedWidth >= getColumnMinWidth(column)
    ) {
      normalized[column.key] = storedWidth;
    }
  });

  return normalized;
}

export function normalizeDataTableState<T extends object>(
  columns: Array<DataTableColumn<T>>,
  state: Partial<DataTableState> | null | undefined
): DataTableState {
  const fallback = {
    visibleColumnKeys: getDefaultVisibleKeys(columns),
    columnWidths: getDefaultColumnWidths(columns)
  };

  if (!state) {
    return fallback;
  }

  const parsedWidths =
    state.columnWidths && typeof state.columnWidths === 'object'
      ? (state.columnWidths as Record<string, unknown>)
      : {};
  const storedVisibleColumnKeys = Array.isArray(state.visibleColumnKeys)
    ? normalizeVisibleKeys(columns, state.visibleColumnKeys)
    : fallback.visibleColumnKeys;
  const defaultVisibleKeys = getDefaultVisibleKeys(columns);
  const visibleColumnKeys = columns
    .map((column) => column.key)
    .filter(
      (key) =>
        storedVisibleColumnKeys.includes(key) ||
        (!Object.prototype.hasOwnProperty.call(parsedWidths, key) &&
          defaultVisibleKeys.includes(key))
    );

  return {
    visibleColumnKeys,
    columnWidths: normalizeColumnWidths(columns, parsedWidths)
  };
}

function readStoredState<T extends object>(
  storageKey: string,
  columns: Array<DataTableColumn<T>>
): DataTableState {
  const fallback = {
    visibleColumnKeys: getDefaultVisibleKeys(columns),
    columnWidths: getDefaultColumnWidths(columns)
  };

  if (typeof window === 'undefined') {
    return fallback;
  }

  const payload = window.localStorage.getItem(storageKey);

  if (!payload) {
    return fallback;
  }

  try {
    return normalizeDataTableState(
      columns,
      JSON.parse(payload) as Partial<DataTableState>
    );
  } catch {
    return fallback;
  }
}

function writeStoredState(storageKey: string, state: DataTableState) {
  if (typeof window === 'undefined') {
    return;
  }

  window.localStorage.setItem(storageKey, JSON.stringify(state));
}

export function usePersistedDataTableConfiguration<T extends object>({
  columns,
  storageKey
}: {
  columns: Array<DataTableColumn<T>>;
  storageKey: string;
}): DataTableConfiguration {
  const storedState = useMemo(
    () => readStoredState(storageKey, columns),
    [columns, storageKey]
  );
  const [draft, setDraft] = useState<{
    storageKey: string;
    state: DataTableState;
  } | null>(null);
  if (draft && draft.storageKey !== storageKey) setDraft(null);
  const state =
    draft?.storageKey === storageKey
      ? normalizeDataTableState(columns, draft.state)
      : storedState;

  const setVisibleColumnKeys = useCallback<Dispatch<SetStateAction<string[]>>>(
    (value) =>
      setDraft((current) => {
        const previous =
          current?.storageKey === storageKey
            ? normalizeDataTableState(columns, current.state)
            : storedState;
        return {
          storageKey,
          state: {
            ...previous,
            visibleColumnKeys:
              typeof value === 'function'
                ? value(previous.visibleColumnKeys)
                : value
          }
        };
      }),
    [columns, storageKey, storedState]
  );
  const setColumnWidths = useCallback<
    Dispatch<SetStateAction<Record<string, number>>>
  >(
    (value) =>
      setDraft((current) => {
        const previous =
          current?.storageKey === storageKey
            ? normalizeDataTableState(columns, current.state)
            : storedState;
        return {
          storageKey,
          state: {
            ...previous,
            columnWidths:
              typeof value === 'function' ? value(previous.columnWidths) : value
          }
        };
      }),
    [columns, storageKey, storedState]
  );

  useEffect(() => {
    // Reading another table never writes the previous table's draft into it.
    if (!draft || draft.storageKey !== storageKey) return;
    writeStoredState(storageKey, normalizeDataTableState(columns, draft.state));
  }, [columns, draft, storageKey]);

  return { ...state, setVisibleColumnKeys, setColumnWidths };
}
