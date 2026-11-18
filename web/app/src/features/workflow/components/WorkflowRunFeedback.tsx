import { Tag, Tooltip, Typography } from 'antd';
import { i18nText } from '../../../shared/i18n/text';
import type { WorkflowRunState } from '../lib/run-state';

const labels = new Map<string, () => string>([
  ['starting', () => i18nText('workflow', 'run.starting')],
  ['queued', () => i18nText('workflow', 'run.queued')],
  ['running', () => i18nText('workflow', 'run.running')],
  ['succeeded', () => i18nText('workflow', 'run.succeeded')],
  ['failed', () => i18nText('workflow', 'run.failed')],
  ['cancelled', () => i18nText('workflow', 'run.cancelled')],
  ['incomplete', () => i18nText('workflow', 'run.incomplete')],
  ['waiting_human', () => i18nText('workflow', 'run.waiting_human')],
  ['waiting_callback', () => i18nText('workflow', 'run.waiting_callback')],
  ['unconfirmed', () => i18nText('workflow', 'run.unconfirmed')]
]);

export function WorkflowRunFeedback({ state }: { state: WorkflowRunState }) {
  if (state.status === 'idle') return null;
  const label = labels.get(state.status)?.() ?? state.status;
  return (
    <output
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 8,
        maxWidth: 360
      }}
    >
      <Tag
        color={
          state.status === 'failed'
            ? 'error'
            : state.status === 'succeeded'
              ? 'success'
              : 'processing'
        }
      >
        {label}
      </Tag>
      {state.failure ? (
        <Tooltip title={state.failure}>
          <Typography.Text type="danger" ellipsis>
            {state.failure}
          </Typography.Text>
        </Tooltip>
      ) : null}
      {state.connectionLost ? (
        <Typography.Text type="warning">
          {i18nText('workflow', 'run.observation_interrupted')}
        </Typography.Text>
      ) : null}
    </output>
  );
}
