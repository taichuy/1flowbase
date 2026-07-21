import {
  BlockUiLoadingShell,
  BlockUiRenderer,
  type BlockRendererActionEvent
} from '@1flowbase/block-renderer';
import {
  Alert,
  Button,
  Empty,
  Space
} from 'antd';

import type { RestrictedBlockRuntimeHostSnapshot } from '../lib/restricted-block-runtime-host';
import { i18nText } from '../../../shared/i18n/text';

export interface RestrictedBlockRuntimePreviewProps {
  snapshot: RestrictedBlockRuntimeHostSnapshot;
  onAction?: (event: BlockRendererActionEvent) => void;
  onRetry?: () => void;
}

export type RestrictedBlockRuntimeActionEvent = BlockRendererActionEvent;

function getStatusView(status: RestrictedBlockRuntimeHostSnapshot['status']): {
  message: string;
  type: 'info' | 'success' | 'warning' | 'error';
} {
  switch (status) {
    case 'idle':
      return {
        message: i18nText('frontstage', 'auto.not_run_yet'),
        type: 'info'
      };
    case 'running':
      return { message: i18nText('frontstage', 'auto.running'), type: 'info' };
    case 'ready':
      return {
        message: i18nText('frontstage', 'auto.run_result'),
        type: 'success'
      };
    case 'failed':
      return {
        message: i18nText('frontstage', 'auto.run_failed'),
        type: 'error'
      };
    case 'timed_out':
      return {
        message: i18nText('frontstage', 'auto.run_timeout'),
        type: 'warning'
      };
    case 'disposed':
      return { message: i18nText('frontstage', 'auto.released'), type: 'info' };
  }
}

export function RestrictedBlockRuntimePreview({
  snapshot,
  onAction,
  onRetry
}: RestrictedBlockRuntimePreviewProps) {
  if (snapshot.status === 'idle' || snapshot.status === 'running') {
    return (
      <div
        data-testid="restricted-block-runtime-preview"
        style={{ width: '100%' }}
      >
        <BlockUiLoadingShell />
      </div>
    );
  }

  const view = getStatusView(snapshot.status);

  return (
    <Space
      data-testid="restricted-block-runtime-preview"
      direction="vertical"
      size="small"
      style={{ width: '100%' }}
    >
      {snapshot.status === 'disposed' ? (
        <Alert type={view.type} showIcon message={view.message} />
      ) : null}

      {snapshot.status === 'ready' ? (
        <Space direction="vertical" size="small" style={{ width: '100%' }}>
          {snapshot.view === undefined ? (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={i18nText('frontstage', 'auto.no_ui_schema')}
            />
          ) : (
            <BlockUiRenderer
              schema={snapshot.view}
              validationOptions={snapshot.schemaValidationOptions}
              onAction={onAction}
            />
          )}
        </Space>
      ) : null}

      {snapshot.status === 'failed' || snapshot.status === 'timed_out' ? (
        <Alert
          type={view.type}
          showIcon
          message={view.message}
          description={snapshot.error?.message}
          action={
            onRetry ? (
              <Button size="small" onClick={onRetry}>
                {i18nText('frontstage', 'auto.retry')}
              </Button>
            ) : null
          }
        />
      ) : null}

    </Space>
  );
}
