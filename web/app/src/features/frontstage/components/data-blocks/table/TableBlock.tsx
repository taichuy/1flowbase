import { useState } from 'react';
import { Alert } from 'antd';
import { i18nText } from '../../../../../shared/i18n/text';
import { useDataBlockMetadata } from '../../../api/data-blocks';
import { useDataBlockHost } from '../../../lib/data-blocks/host-context';
import {
  readDataBlockConfig,
  type DataBlockComponentProps,
  type TableBlockConfig
} from '../../../lib/data-blocks/types';
import { useTableConfigController } from './use-table-config-controller';
import { useTableSession } from './runtime/use-table-session';
import { useTableActions } from './runtime/use-table-actions';
import { TableBlockView } from './view/TableBlockView';

export function TableBlock({ ctx }: DataBlockComponentProps) {
  const config = readDataBlockConfig(ctx.props);
  if (config?.kind !== 'table')
    return (
      <Alert
        type="error"
        showIcon
        title={i18nText('frontstage', 'data_blocks.table.invalid_config')}
      />
    );
  return (
    <ConfiguredTableBlock
      key={config.binding.model_id}
      ctx={ctx}
      initialConfig={config}
    />
  );
}

function ConfiguredTableBlock({
  ctx,
  initialConfig
}: DataBlockComponentProps & { initialConfig: TableBlockConfig }) {
  const host = useDataBlockHost();
  const [feedback, setFeedback] =
    useState<ReturnType<typeof useTableActions>['feedback']>(null);
  const metadata = useDataBlockMetadata(ctx, initialConfig.binding);
  const configuration = useTableConfigController(
    initialConfig,
    host,
    metadata.data?.fields ?? [],
    () => setFeedback(null),
    () =>
      setFeedback({
        type: 'error',
        text: i18nText('frontstage', 'data_blocks.table.save_failed')
      })
  );
  const session = useTableSession(ctx, configuration.config, metadata);
  const actions = useTableActions(ctx, host, configuration.config, session);
  return (
    <TableBlockView
      ctx={ctx}
      host={host}
      configuration={configuration}
      session={session}
      actions={actions}
      metadata={metadata}
      feedback={actions.feedback ?? feedback}
      onDismissFeedback={() => {
        setFeedback(null);
        actions.dismissFeedback();
      }}
    />
  );
}
