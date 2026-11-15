import { type ReactNode, useState } from 'react';
import { Segmented } from 'antd';
import type { ConversationLogTraceLoader } from '../../conversation-log-trace-model';
import { i18nText } from '../../../../../../shared/i18n/text';
import { NativeTrajectoryWorkspace } from '../NativeTrajectoryWorkspace';
import './workflow-nodes.css';

export function WorkflowRunTrace({
  runId,
  loader,
  children
}: {
  runId: string;
  loader: ConversationLogTraceLoader;
  children: ReactNode;
}) {
  const [view, setView] = useState<'nodes' | 'trajectory'>('nodes');
  return (
    <>
      <Segmented
        value={view}
        onChange={setView}
        options={[
          {
            value: 'nodes',
            label: i18nText('agentFlow', 'auto.tracking_nodes')
          },
          {
            value: 'trajectory',
            label: i18nText('agentFlow', 'trajectory.run_title')
          }
        ]}
      />
      {view === 'nodes' ? (
        <div className="workflow-node-trace">{children}</div>
      ) : (
        <NativeTrajectoryWorkspace runId={runId} loader={loader} />
      )}
    </>
  );
}
