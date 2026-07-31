import { useEffect, useState } from 'react';
import type { Dispatch, ReactNode, SetStateAction } from 'react';

export const DEFAULT_MIN_COLUMN_WIDTH = 110;

export type DataTableColumn<T extends object> = {
  key: string;
  title: string;
  width: number;
  minWidth?: number;
  dataIndex?: keyof T;
  render?: (value: unknown, record: T, index: number) => ReactNode;
  ellipsis?: boolean;
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
  return columns.map((column) => column.key);
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
  const normalized = defaultVisibleKeys.filter((key) =>
    visibleColumnKeys.includes(key)
  );

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
  const visibleColumnKeys = getDefaultVisibleKeys(columns).filter(
    (key) =>
      storedVisibleColumnKeys.includes(key) ||
      !Object.prototype.hasOwnProperty.call(parsedWidths, key)
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
  const [visibleColumnKeys, setVisibleColumnKeys] = useState<string[]>(() => {
    return readStoredState(storageKey, columns).visibleColumnKeys;
  });
  const [columnWidths, setColumnWidths] = useState<Record<string, number>>(
    () => {
      return readStoredState(storageKey, columns).columnWidths;
    }
  );

  useEffect(() => {
    const state = readStoredState(storageKey, columns);

    setVisibleColumnKeys(state.visibleColumnKeys);
    setColumnWidths(state.columnWidths);
  }, [columns, storageKey]);

  useEffect(() => {
    writeStoredState(storageKey, {
      visibleColumnKeys,
      columnWidths
    });
  }, [columnWidths, storageKey, visibleColumnKeys]);

  return {
    visibleColumnKeys,
    columnWidths,
    setVisibleColumnKeys,
    setColumnWidths
  };
}
