import type {
  DataBlockColumn,
  TableBlockConfig
} from '../../../lib/data-blocks/types';

export type TableColumnEntry =
  | { id: string; kind: 'field'; column: DataBlockColumn }
  | { id: 'actions'; kind: 'actions'; hidden: boolean };

export function tableColumnEntries(
  config: TableBlockConfig
): TableColumnEntry[] {
  const entries: TableColumnEntry[] = config.columns.map((column) => ({
    id: `field:${column.field_code}`,
    kind: 'field',
    column
  }));
  entries.splice(
    Math.max(0, Math.min(config.action_column?.position ?? 0, entries.length)),
    0,
    {
      id: 'actions',
      kind: 'actions',
      hidden:
        config.action_column?.hidden ??
        !config.row_actions.some((action) => action.enabled)
    }
  );
  return entries;
}

export function moveTableColumn(
  config: TableBlockConfig,
  active: string,
  over: string
): TableBlockConfig {
  const entries = tableColumnEntries(config);
  const from = entries.findIndex((entry) => entry.id === active);
  const to = entries.findIndex((entry) => entry.id === over);
  if (from < 0 || to < 0 || from === to) return config;
  entries.splice(to, 0, entries.splice(from, 1)[0]!);
  const actions = entries.find((entry) => entry.kind === 'actions')!;
  return {
    ...config,
    columns: entries.flatMap((entry) =>
      entry.kind === 'field' ? [entry.column] : []
    ),
    action_column: {
      hidden: actions.kind === 'actions' && actions.hidden,
      position: entries.indexOf(actions)
    }
  };
}
