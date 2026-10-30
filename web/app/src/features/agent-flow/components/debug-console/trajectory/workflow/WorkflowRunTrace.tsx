import { type ReactNode, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Alert, Button, Segmented, Spin } from 'antd';
import type { ConversationLogTraceLoader } from '../../conversation-log-trace-model';
import { i18nText } from '../../../../../../shared/i18n/text';
import { NativeTrajectoryWorkspace } from '../NativeTrajectoryWorkspace';

/** Run-level Workflow entry; the backend index determines whether rounds exist. */
export function WorkflowRunTrace({
  runId,
  loader,
  children
}: {
  runId: string;
  loader: ConversationLogTraceLoader;
  children: ReactNode;
}) {
  const [view, setView] = useState<'trajectory' | 'nodes'>('trajectory');
  const index = useQuery({
    // Share the unfiltered index with NativeTrajectoryWorkspace, including events
    // outside the first row window. Never infer nesting from duplicate node IDs.
    queryKey: ['workflow-trajectory', runId, { category: 'all' }, 'index'],
    queryFn: () =>
      loader.loadWorkflowTrajectory!(runId, undefined, {
        category: 'all',
        index_only: true
      }),
    staleTime: 60_000,
    refetchOnWindowFocus: false,
    retry: false
  });
  if (index.isPending) return <Spin />;
  if (index.isError)
    return (
      <>
        <Alert
          type="error"
          title={i18nText('agentFlow', 'auto.loading_failed')}
          action={
            <Button onClick={() => void index.refetch()}>
              {i18nText('agentFlow', 'auto.retry')}
            </Button>
          }
        />
        {children}
      </>
    );
  const hasContainers =
    index.data.nodes.some(
      (node) => node.node_type === 'loop' || node.node_type === 'iteration'
    ) ||
    index.data.overview?.items.some((event) =>
      Boolean(event.execution_identity?.round_path.length)
    );
  if (!hasContainers) return children;
  return (
    <>
      <Segmented
        value={view}
        onChange={setView}
        options={[
          {
            value: 'trajectory',
            label: i18nText('agentFlow', 'trajectory.run_title')
          },
          {
            value: 'nodes',
            label: i18nText('agentFlow', 'auto.tracking_nodes')
          }
        ]}
      />
      {view === 'trajectory' ? (
        <NativeTrajectoryWorkspace runId={runId} loader={loader} />
      ) : (
        children
      )}
    </>
  );
}
