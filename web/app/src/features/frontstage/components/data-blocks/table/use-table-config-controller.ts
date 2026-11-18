import { useRef, useState } from 'react';
import type { DataBlockHost } from '../../../lib/data-blocks/host-context';
import type {
  DataBlockAction,
  DataBlockColumn,
  DataBlockField,
  TableBlockConfig
} from '../../../lib/data-blocks/types';
import { persistTableConfig, prepareTableAction } from './table-model';

/** Owns acknowledged design writes; incoming host props remain the configuration authority. */
export function useTableConfigController(
  initialConfig: TableBlockConfig,
  host: DataBlockHost | null,
  fields: DataBlockField[],
  onSaved: () => void,
  onSaveFailed: () => void
) {
  const [acknowledged, setAcknowledged] = useState<{
    source: TableBlockConfig;
    config: TableBlockConfig;
  } | null>(null);
  const config =
    acknowledged?.source === initialConfig
      ? acknowledged.config
      : initialConfig;
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);

  const commit = async (
    next: () => Promise<TableBlockConfig> | TableBlockConfig
  ) => {
    if (!host?.design_mode || savingRef.current || host.saving) return false;
    savingRef.current = true;
    setSaving(true);
    try {
      const candidate = await next();
      await persistTableConfig(host, candidate, (committed) => {
        setAcknowledged({ source: initialConfig, config: committed });
      });
      onSaved();
      return true;
    } catch {
      onSaveFailed();
      return false;
    } finally {
      savingRef.current = false;
      setSaving(false);
    }
  };
  const saveConfig = (next: TableBlockConfig) =>
    commit(async () => {
      if (
        JSON.stringify([config.relation, config.linkage]) ===
        JSON.stringify([next.relation, next.linkage])
      )
        return next;
      const actions: DataBlockAction[] = [];
      const row_actions: DataBlockAction[] = [];
      const columns: DataBlockColumn[] = [];
      // Child targets share the host tree; prepare them sequentially.
      for (const action of next.actions)
        actions.push(await prepareTableAction(host!, next, action, fields));
      for (const action of next.row_actions)
        row_actions.push(await prepareTableAction(host!, next, action, fields));
      for (const column of next.columns)
        columns.push(
          column.action
            ? {
                ...column,
                action: await prepareTableAction(
                  host!,
                  next,
                  column.action,
                  fields
                )
              }
            : column
        );
      return { ...next, actions, row_actions, columns };
    });
  const saveAction = (action: DataBlockAction, row: boolean) =>
    commit(async () => {
      const prepared = await prepareTableAction(host!, config, action, fields);
      const key = row ? 'row_actions' : 'actions';
      const exists = config[key].some((item) => item.id === action.id);
      return {
        ...config,
        [key]: exists
          ? config[key].map((item) => (item.id === action.id ? prepared : item))
          : [...config[key], prepared]
      };
    });
  const saveColumn = (column: DataBlockColumn) =>
    commit(async () => ({
      ...config,
      columns: await Promise.all(
        config.columns.map(async (item) =>
          item.field_code === column.field_code
            ? {
                ...column,
                ...(column.action
                  ? {
                      action: await prepareTableAction(
                        host!,
                        config,
                        column.action,
                        fields
                      )
                    }
                  : {})
              }
            : item
        )
      )
    }));

  return { config, saving, saveConfig, saveAction, saveColumn };
}
