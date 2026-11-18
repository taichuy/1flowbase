import CheckCircleFilled from '@ant-design/icons/es/icons/CheckCircleFilled';
import CloseCircleFilled from '@ant-design/icons/es/icons/CloseCircleFilled';
import LoadingOutlined from '@ant-design/icons/es/icons/LoadingOutlined';

import { i18nText } from '../../../../shared/i18n/text';
import type { AgentFlowCanvasNodeData } from '../canvas/node-types';
import './node-run-status.css';

export function NodeRunStatus({
  status
}: {
  status: AgentFlowCanvasNodeData['runStatus'];
}) {
  if (!status) return null;
  const label =
    status === 'running'
      ? i18nText('agentFlow', 'auto.running')
      : status === 'succeeded'
        ? i18nText('agentFlow', 'auto.executed_successfully')
        : i18nText('agentFlow', 'auto.execution_failed');
  return (
    <span
      className="agent-flow-node-run-status"
      data-status={status}
      role="img"
      aria-label={label}
      title={label}
    >
      {status === 'running' ? (
        <LoadingOutlined spin />
      ) : status === 'succeeded' ? (
        <CheckCircleFilled />
      ) : (
        <CloseCircleFilled />
      )}
    </span>
  );
}
