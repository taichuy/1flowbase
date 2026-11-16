import { useState } from 'react';
import type { BlockContext } from '@1flowbase/page-protocol';
import type { ConsoleRuntimeModelRecord } from '@1flowbase/api-client';
import { i18nText } from '../../../../../../shared/i18n/text';
import { useDataBlockWrites } from '../../../../api/data-blocks';
import type { DataBlockHost } from '../../../../lib/data-blocks/host-context';
import type {
  DataBlockAction,
  TableBlockConfig
} from '../../../../lib/data-blocks/types';
import {
  deleteTableRecords,
  tableActionAllowed,
  tableActionInputs,
  tableActionParentContext,
  tableRecordId
} from '../table-model';
import type { useTableSession } from './use-table-session';

export function useTableActions(
  ctx: BlockContext,
  host: DataBlockHost | null,
  config: TableBlockConfig,
  session: ReturnType<typeof useTableSession>
) {
  const [pendingDelete, setPendingDelete] = useState<{
    contextKey: string;
    ids: string[];
    deleting: boolean;
  } | null>(null);
  const [reported, setReported] = useState<{
    contextKey: string;
    value: { type: 'error' | 'success' | 'warning'; text: string };
  } | null>(null);
  const activeDelete =
    pendingDelete?.contextKey === session.contextKey ? pendingDelete : null;
  const feedback =
    reported?.contextKey === session.contextKey ? reported.value : null;
  const writes = useDataBlockWrites(ctx, config.binding);
  const actionParent = tableActionParentContext(
    config,
    session.fields,
    ctx.inputs
  );
  const report = (type: 'error' | 'success' | 'warning', text: string) => {
    setReported({ contextKey: session.contextKey, value: { type, text } });
  };
  const runAction = async (
    action: DataBlockAction,
    record?: ConsoleRuntimeModelRecord
  ) => {
    if (!tableActionAllowed(action, session.capabilities)) return;
    const recordId = record
      ? tableRecordId(record, session.idField)
      : undefined;
    if (record && !recordId) return;
    if ((record || action.kind === 'delete') && !session.canAddressRecords)
      return;
    if (action.kind === 'delete') {
      const ids = recordId ? [recordId] : session.selected;
      if (ids.length)
        setPendingDelete({
          contextKey: session.contextKey,
          ids,
          deleting: false
        });
      return;
    }
    if (action.kind === 'refresh') {
      if (
        session.scope.enabled &&
        !session.invalidQuery &&
        session.capabilities?.can_list
      )
        await session.records.refetch();
      return;
    }
    if (action.kind === 'filter') return;
    if (action.kind === 'create' && !actionParent.canCreate) return;
    if (!host || !action.target_block_id) {
      report(
        'error',
        i18nText('frontstage', 'data_blocks.table.action_unavailable')
      );
      return;
    }
    try {
      await host.openAction(
        action,
        tableActionInputs(
          { ...config, relation: actionParent.relation },
          host.block_id,
          actionParent.inputs,
          recordId ?? undefined
        )
      );
    } catch {
      report(
        'error',
        i18nText('frontstage', 'data_blocks.table.action_failed')
      );
    }
  };
  const confirmDelete = async () => {
    if (
      !activeDelete?.ids.length ||
      !session.capabilities?.can_delete ||
      activeDelete.deleting
    )
      return;
    setPendingDelete({ ...activeDelete, deleting: true });
    const result = await deleteTableRecords(activeDelete.ids, (id) =>
      writes.remove.mutateAsync(id)
    );
    session.completeDelete({
      contextKey: session.contextKey,
      requestKey: session.requestKey,
      visibleCount: session.data.length,
      ...result
    });
    setPendingDelete((current) =>
      current?.contextKey === session.contextKey ? null : current
    );
    report(
      result.failed.length ? 'warning' : 'success',
      result.failed.length
        ? i18nText('frontstage', 'data_blocks.table.delete_partial', {
            deleted: result.deleted.length,
            failed: result.failed.length
          })
        : i18nText('frontstage', 'data_blocks.table.delete_success', {
            count: result.deleted.length
          })
    );
  };

  return {
    actionParent,
    feedback,
    dismissFeedback() {
      setReported(null);
    },
    deleteIds: activeDelete?.ids ?? null,
    deleting: activeDelete?.deleting ?? false,
    cancelDelete() {
      if (!activeDelete?.deleting) setPendingDelete(null);
    },
    runAction,
    confirmDelete
  };
}
