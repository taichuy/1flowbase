import type { ReactNode } from 'react';
import { Alert, Button, Empty, Spin } from 'antd';
import { i18nText } from '../../../../../../shared/i18n/text';
import type { useDataBlockMetadata } from '../../../../api/data-blocks';
import type { useTableSession } from '../runtime/use-table-session';

export function TableState({
  metadata,
  session,
  fieldMenu,
  design,
  children
}: {
  metadata: ReturnType<typeof useDataBlockMetadata>;
  session: ReturnType<typeof useTableSession>;
  fieldMenu: ReactNode;
  design: boolean;
  children: ReactNode;
}) {
  const { capabilities, scope, invalidQuery, records, missingRecordId } =
    session;
  let state: ReactNode = null;
  if (metadata.isPending) state = <Spin />;
  else if (metadata.isError)
    state = (
      <Alert
        type="error"
        showIcon
        title={i18nText('frontstage', 'data_blocks.table.metadata_failed')}
        action={
          <Button size="small" onClick={() => void metadata.refetch()}>
            {i18nText('frontstage', 'data_blocks.table.retry')}
          </Button>
        }
      />
    );
  else if (!capabilities?.can_list)
    state = (
      <Alert
        type="warning"
        showIcon
        title={i18nText('frontstage', 'data_blocks.table.list_forbidden')}
      />
    );
  else if (!scope.enabled)
    state = (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={i18nText(
          'frontstage',
          'data_blocks.table.awaiting_source'
        )}
      />
    );
  else if (invalidQuery)
    state = (
      <Alert
        type="warning"
        showIcon
        title={i18nText('frontstage', 'data_blocks.table.invalid_query_fields')}
      />
    );
  else if (records.isError)
    state = (
      <Alert
        type="error"
        showIcon
        title={i18nText('frontstage', 'data_blocks.table.records_failed')}
        action={
          <Button size="small" onClick={() => void records.refetch()}>
            {i18nText('frontstage', 'data_blocks.table.retry')}
          </Button>
        }
      />
    );
  else if (missingRecordId)
    state = (
      <Alert
        type="error"
        showIcon
        title={i18nText('frontstage', 'data_blocks.table.missing_record_id')}
      />
    );

  if (!state) return children;
  return (
    <>
      <div className="data-table-state">{state}</div>
      {design && <div className="data-table-state-config">{fieldMenu}</div>}
    </>
  );
}
