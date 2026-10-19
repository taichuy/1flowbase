import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import { patchUserPreferences } from '../../user-preferences/user-preferences';
import {
  normalizeDataTableState,
  type DataTableColumn,
  type DataTableConfiguration,
  type DataTableState
} from './data-table-state';

const DATA_TABLES_META_PATH = ['ui', 'data_tables'] as const;
const SAVE_DELAY_MS = 250;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function readDataTableStateFromMeta<T extends object>(
  meta: Record<string, unknown> | undefined,
  preferenceKey: string,
  columns: Array<DataTableColumn<T>>
): DataTableState {
  const uiValue = meta?.[DATA_TABLES_META_PATH[0]];
  const ui = isRecord(uiValue) ? uiValue : undefined;
  const dataTablesValue = ui?.[DATA_TABLES_META_PATH[1]];
  const dataTables = isRecord(dataTablesValue) ? dataTablesValue : undefined;
  const tableState = isRecord(dataTables?.[preferenceKey])
    ? (dataTables[preferenceKey] as Partial<DataTableState>)
    : undefined;

  return normalizeDataTableState(columns, tableState);
}

function buildDataTableMetaPatch(
  preferenceKey: string,
  state: DataTableState
): Record<string, unknown> {
  return {
    ui: {
      data_tables: {
        [preferenceKey]: state
      }
    }
  };
}

export function useUserPreferenceDataTableConfiguration<T extends object>({
  columns,
  preferenceKey
}: {
  columns: Array<DataTableColumn<T>>;
  preferenceKey: string;
}): DataTableConfiguration {
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const me = useAuthStore((state) => state.me);
  const setMe = useAuthStore((state) => state.setMe);
  const owner = JSON.stringify([me?.id ?? null, preferenceKey]);
  const storedState = useMemo(
    () => readDataTableStateFromMeta(me?.meta, preferenceKey, columns),
    [columns, me?.meta, preferenceKey]
  );
  const [draft, setDraft] = useState<{
    owner: string;
    state: DataTableState;
  } | null>(null);
  const saveQueue = useRef<Promise<void>>(Promise.resolve());
  if (draft && draft.owner !== owner) setDraft(null);
  const state =
    draft?.owner === owner
      ? normalizeDataTableState(columns, draft.state)
      : storedState;

  const setVisibleColumnKeys = useCallback<Dispatch<SetStateAction<string[]>>>(
    (value) =>
      setDraft((current) => {
        const previous =
          current?.owner === owner
            ? normalizeDataTableState(columns, current.state)
            : storedState;
        return {
          owner,
          state: {
            ...previous,
            visibleColumnKeys:
              typeof value === 'function'
                ? value(previous.visibleColumnKeys)
                : value
          }
        };
      }),
    [columns, owner, storedState]
  );
  const setColumnWidths = useCallback<
    Dispatch<SetStateAction<Record<string, number>>>
  >(
    (value) =>
      setDraft((current) => {
        const previous =
          current?.owner === owner
            ? normalizeDataTableState(columns, current.state)
            : storedState;
        return {
          owner,
          state: {
            ...previous,
            columnWidths:
              typeof value === 'function' ? value(previous.columnWidths) : value
          }
        };
      }),
    [columns, owner, storedState]
  );

  useEffect(() => {
    // Only setters create a draft; server/meta hydration cannot schedule a PATCH.
    if (!draft || draft.owner !== owner || !csrfToken) return;
    const userId = me?.id;
    const nextState = normalizeDataTableState(columns, draft.state);
    let active = true;
    const timeout = window.setTimeout(() => {
      // Preserve edit order when a later debounce expires during an earlier request.
      saveQueue.current = saveQueue.current.then(async () => {
        if (!active) return;
        try {
          const updatedMe = await patchUserPreferences(
            buildDataTableMetaPatch(preferenceKey, nextState),
            csrfToken
          );
          const currentAuth = useAuthStore.getState();
          if (
            !active ||
            currentAuth.me?.id !== userId ||
            currentAuth.csrfToken !== csrfToken
          )
            return;
          setMe(updatedMe);
          setDraft((current) => (current === draft ? null : current));
        } catch {
          // Preferences are non-critical UI state; retain the user's local edit.
        }
      });
    }, SAVE_DELAY_MS);
    return () => {
      active = false;
      window.clearTimeout(timeout);
    };
  }, [columns, csrfToken, draft, me?.id, owner, preferenceKey, setMe]);

  return { ...state, setVisibleColumnKeys, setColumnWidths };
}
