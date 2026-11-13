import CheckCircleFilled from '@ant-design/icons/es/icons/CheckCircleFilled';
import CloseCircleFilled from '@ant-design/icons/es/icons/CloseCircleFilled';
import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';
import LoadingOutlined from '@ant-design/icons/es/icons/LoadingOutlined';
import { Button, Descriptions, Typography } from 'antd';

import { i18nText } from '../../../shared/i18n/text';
import type { WorkflowRunState } from '../lib/run-state';
import './workflow-test-run.css';

function RunResult({ value }: { value: unknown }) {
  if (Array.isArray(value))
    return (
      <div>
        {value.map((entry, i) => (
          <RunResult key={i} value={entry} />
        ))}
      </div>
    );
  if (value && typeof value === 'object')
    return (
      <Descriptions
        column={1}
        size="small"
        items={Object.entries(value).map(([key, entry]) => ({
          key,
          label: key,
          children: <RunResult value={entry} />
        }))}
      />
    );
  return (
    <Typography.Text>{value == null ? '—' : String(value)}</Typography.Text>
  );
}

function statusLabel(state: WorkflowRunState) {
  if (state.connectionLost)
    return i18nText('workflow', 'auto.workflow_test_run_connection_lost');
  switch (state.status) {
    case 'succeeded':
      return i18nText('workflow', 'auto.workflow_test_run_status_succeeded');
    case 'failed':
      return i18nText('workflow', 'auto.workflow_test_run_status_failed');
    case 'cancelled':
      return i18nText('workflow', 'auto.workflow_test_run_status_cancelled');
    case 'incomplete':
      return i18nText('workflow', 'auto.workflow_test_run_status_incomplete');
    case 'waiting_human':
    case 'waiting_callback':
      return i18nText('workflow', 'auto.workflow_test_run_status_waiting');
    default:
      return i18nText('workflow', 'auto.workflow_test_run_status_running');
  }
}

export function WorkflowRunStatusBar({
  state,
  active,
  onOpenTrace,
  onClear
}: {
  state: WorkflowRunState;
  active: boolean;
  onOpenTrace: (runId: string | null) => void;
  onClear: () => void;
}) {
  if (state.status === 'idle') return null;
  return (
    <section
      className="workflow-run-status"
      data-status={state.status}
      aria-label={i18nText('workflow', 'auto.workflow_test_run_status')}
    >
      <div className="workflow-run-status__summary" role="status">
        <span className="workflow-run-status__icon">
          {active ? (
            <LoadingOutlined spin />
          ) : state.status === 'succeeded' ? (
            <CheckCircleFilled />
          ) : state.status === 'failed' ? (
            <CloseCircleFilled />
          ) : null}
        </span>
        <Typography.Text strong>{statusLabel(state)}</Typography.Text>
        <Button
          type="link"
          size="small"
          onClick={() => onOpenTrace(state.runId)}
        >
          {i18nText('workflow', 'auto.workflow_test_run_view_trace')}
        </Button>
        {!active ? (
          <Button
            type="text"
            size="small"
            icon={<CloseOutlined />}
            onClick={onClear}
            aria-label={i18nText('workflow', 'auto.workflow_test_run_clear')}
          />
        ) : null}
      </div>
      {state.failure ? (
        <Typography.Text type="danger">{state.failure}</Typography.Text>
      ) : null}
      {!active && state.output !== null ? (
        <details className="workflow-run-status__result">
          <summary>{i18nText('workflow', 'auto.workflow_result')}</summary>
          <div className="workflow-run-status__result-content">
            {Object.keys(state.output).length ? (
              <RunResult value={state.output} />
            ) : (
              <Typography.Text type="secondary">
                {i18nText('workflow', 'auto.workflow_test_run_empty_result')}
              </Typography.Text>
            )}
          </div>
        </details>
      ) : null}
    </section>
  );
}
