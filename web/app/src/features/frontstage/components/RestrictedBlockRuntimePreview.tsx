import {
  BlockUiRenderer,
  type BlockRendererActionEvent
} from '@1flowbase/block-renderer';
import { Alert, Descriptions, Empty, Space, Tag, Typography } from 'antd';

import type { RestrictedBlockRuntimeHostSnapshot } from '../lib/restricted-block-runtime-host';
import { i18nText } from '../../../shared/i18n/text';

export interface RestrictedBlockRuntimePreviewProps {
  snapshot: RestrictedBlockRuntimeHostSnapshot;
  onAction?: (event: BlockRendererActionEvent) => void;
}

export type RestrictedBlockRuntimeActionEvent = BlockRendererActionEvent;

function getStatusView(status: RestrictedBlockRuntimeHostSnapshot['status']): {
  message: string;
  type: 'info' | 'success' | 'warning' | 'error';
} {
  switch (status) {
    case 'idle':
      return { message: i18nText("frontstage", "auto.not_run_yet"), type: 'info' };
    case 'running':
      return { message: i18nText("frontstage", "auto.running"), type: 'info' };
    case 'ready':
      return { message: i18nText("frontstage", "auto.run_result"), type: 'success' };
    case 'failed':
      return { message: i18nText("frontstage", "auto.run_failed"), type: 'error' };
    case 'timed_out':
      return { message: i18nText("frontstage", "auto.run_timeout"), type: 'warning' };
    case 'disposed':
      return { message: i18nText("frontstage", "auto.released"), type: 'info' };
  }
}

export function RestrictedBlockRuntimePreview({
  snapshot,
  onAction
}: RestrictedBlockRuntimePreviewProps) {
  const view = getStatusView(snapshot.status);

  return (
    <Space
      data-testid="restricted-block-runtime-preview"
      direction="vertical"
      size="small"
      style={{ width: '100%' }}
    >
      <Alert type={view.type} showIcon message={view.message} />

      {snapshot.status === 'ready' ? (
        <Space direction="vertical" size="small" style={{ width: '100%' }}>
          {snapshot.schema === undefined ? (
            <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={i18nText("frontstage", "auto.no_ui_schema")} />
          ) : (
            <BlockUiRenderer
              schema={snapshot.schema}
              validationOptions={snapshot.schemaValidationOptions}
              onAction={onAction}
            />
          )}
        </Space>
      ) : null}

      {snapshot.status === 'failed' || snapshot.status === 'timed_out' ? (
        <RuntimeErrorSummary snapshot={snapshot} />
      ) : null}

      <RuntimeActivitySummary snapshot={snapshot} />
    </Space>
  );
}

function RuntimeErrorSummary({
  snapshot
}: {
  snapshot: RestrictedBlockRuntimeHostSnapshot;
}) {
  const firstError = snapshot.error?.errors[0];

  return (
    <Descriptions
      bordered
      size="small"
      column={1}
      title={i18nText("frontstage", "auto.error_summary")}
      items={[
        {
          key: 'kind',
          label: i18nText("frontstage", "auto.kind"),
          children: snapshot.error?.kind ?? i18nText("frontstage", "auto.unknown")
        },
        {
          key: 'message',
          label: i18nText("frontstage", "auto.message"),
          children: snapshot.error?.message ?? i18nText("frontstage", "auto.runtime_failed")
        },
        {
          key: 'code',
          label: i18nText("frontstage", "auto.code"),
          children: firstError?.code ?? snapshot.error?.kind ?? i18nText("frontstage", "auto.runtime_error")
        },
        {
          key: 'path',
          label: i18nText("frontstage", "auto.path"),
          children: firstError?.path ?? i18nText("frontstage", "auto.runtime")
        }
      ]}
    />
  );
}

function RuntimeActivitySummary({
  snapshot
}: {
  snapshot: RestrictedBlockRuntimeHostSnapshot;
}) {
  return (
    <Space direction="vertical" size="small" style={{ width: '100%' }}>
      <Typography.Text strong>{i18nText("frontstage", "auto.logs")}</Typography.Text>
      {snapshot.logs.length > 0 ? (
        <Space direction="vertical" size={4} style={{ width: '100%' }}>
          <Typography.Text type="secondary">
            {snapshot.logs.length} {i18nText("frontstage", "auto.item_count_suffix")}</Typography.Text>
          {snapshot.logs.map((log, index) => (
            <Typography.Text key={`${log.level}-${index}`}>
              <Tag>{log.level}</Tag>
              {log.message}
            </Typography.Text>
          ))}
        </Space>
      ) : (
        <Typography.Text type="secondary">{i18nText("frontstage", "auto.none")}</Typography.Text>
      )}

      <Typography.Text strong>{i18nText("frontstage", "auto.effects")}</Typography.Text>
      {snapshot.effects.length > 0 ? (
        <Space direction="vertical" size={4} style={{ width: '100%' }}>
          <Typography.Text type="secondary">
            {snapshot.effects.length} {i18nText("frontstage", "auto.effect_count_suffix")}</Typography.Text>
          {snapshot.effects.map((effect, index) => (
            <Typography.Text key={`${effect.type}-${index}`}>
              {formatEffect(effect)}
            </Typography.Text>
          ))}
        </Space>
      ) : (
        <Typography.Text type="secondary">{i18nText("frontstage", "auto.none")}</Typography.Text>
      )}

      <Typography.Text strong>{i18nText("frontstage", "auto.rejections")}</Typography.Text>
      {snapshot.rejections.length > 0 ? (
        <Space direction="vertical" size={4} style={{ width: '100%' }}>
          <Typography.Text type="secondary">
            {snapshot.rejections.length} {i18nText("frontstage", "auto.rejection_count_suffix")}</Typography.Text>
          {snapshot.rejections.map((rejection, index) => (
            <Typography.Text key={`${rejection.code}-${index}`}>
              <Tag color="warning">{rejection.code}</Tag>
              {rejection.path}: {rejection.message}
            </Typography.Text>
          ))}
        </Space>
      ) : (
        <Typography.Text type="secondary">{i18nText("frontstage", "auto.none")}</Typography.Text>
      )}
    </Space>
  );
}

function formatEffect(
  effect: RestrictedBlockRuntimeHostSnapshot['effects'][number]
): string {
  switch (effect.type) {
    case 'action':
      return `${i18nText("frontstage", "auto.effect_action")}: ${effect.actionId}`;
    case 'data':
      return `${i18nText("frontstage", "auto.effect_data")}: ${effect.queryId}`;
    case 'event':
      return `${i18nText("frontstage", "auto.effect_event")}: ${effect.name}`;
  }
}
