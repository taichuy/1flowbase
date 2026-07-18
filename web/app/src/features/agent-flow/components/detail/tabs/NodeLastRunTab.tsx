import { useQuery } from '@tanstack/react-query';
import { Result } from 'antd';

import type { CanvasNodeSchema } from '../../../../../shared/schema-ui/v1/contracts/canvas-node-schema';
import { SchemaRenderer } from '../../../../../shared/schema-ui/v1/runtime/SchemaRenderer';
import type { SchemaAdapter } from '../../../../../shared/schema-ui/v1/registry/create-renderer-registry';

import {
  applicationRunNodeLastRunQueryKey,
  fetchApplicationRunNodeLastRun,
  fetchNodeLastRun,
  nodeLastRunQueryKey,
  type NodeLastRun
} from '../../../api/runtime';
import { agentFlowRendererRegistry } from '../../../schema/agent-flow-renderer-registry';
import { NodeRunIOCard } from '../last-run/NodeRunIOCard';
import { NodeRunMetadataCard } from '../last-run/NodeRunMetadataCard';
import { NodeRunSummaryCard } from '../last-run/NodeRunSummaryCard';
import { NodeRunEmptyState } from '../last-run/NodeRunEmptyState';
import { i18nText } from '../../../../../shared/i18n/text';

function isNodeLastRun(value: unknown): value is NonNullable<
  Awaited<ReturnType<typeof fetchNodeLastRun>>
> {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const candidate = value as Record<string, unknown>;

  return Boolean(
    candidate.flow_run &&
      typeof candidate.flow_run === 'object' &&
      candidate.node_run &&
      typeof candidate.node_run === 'object' &&
      Array.isArray(candidate.events)
  );
}

function createLastRunAdapter({
  adapter,
  lastRun,
  emptyDescription
}: {
  adapter: SchemaAdapter;
  lastRun: NodeLastRun | null;
  emptyDescription: string;
}): SchemaAdapter {
  return {
    ...adapter,
    getDerived(key: string) {
      if (key === 'lastRun') {
        return lastRun;
      }

      if (key === 'lastRunEmptyDescription') {
        return emptyDescription;
      }

      return adapter.getDerived(key);
    }
  };
}

function renderLastRunContent({
  schema,
  adapter,
  lastRun,
  emptyDescription
}: {
  schema?: CanvasNodeSchema;
  adapter?: SchemaAdapter;
  lastRun: NodeLastRun | null;
  emptyDescription: string;
}) {
  if (schema && adapter) {
    return (
      <div className="agent-flow-node-detail__last-run">
        <SchemaRenderer
          adapter={createLastRunAdapter({ adapter, lastRun, emptyDescription })}
          blocks={schema.detail.tabs.lastRun.blocks}
          capabilities={schema.capabilities}
          registry={agentFlowRendererRegistry}
        />
      </div>
    );
  }

  if (!lastRun) {
    return (
      <div className="agent-flow-node-detail__last-run">
        <NodeRunEmptyState description={emptyDescription} />
      </div>
    );
  }

  return (
    <div className="agent-flow-node-detail__last-run">
      <NodeRunSummaryCard lastRun={lastRun} />
      <NodeRunIOCard lastRun={lastRun} />
      <NodeRunMetadataCard lastRun={lastRun} />
    </div>
  );
}

export function NodeLastRunTab({
  activeRunId,
  applicationId,
  nodeId,
  onResolveRunScope,
  schema,
  adapter
}: {
  activeRunId?: string | null;
  applicationId?: string;
  nodeId?: string;
  onResolveRunScope?: ((runId: string | null) => void) | undefined;
  schema?: CanvasNodeSchema;
  adapter?: SchemaAdapter;
}) {
  const lastRunQuery = useQuery({
    queryKey: activeRunId
      ? applicationRunNodeLastRunQueryKey(
          applicationId ?? 'unknown',
          activeRunId,
          nodeId ?? 'unknown'
        )
      : nodeLastRunQueryKey(applicationId ?? 'unknown', nodeId ?? 'unknown'),
    queryFn: async () => {
      if (activeRunId) {
        return fetchApplicationRunNodeLastRun(
          applicationId!,
          activeRunId,
          nodeId!
        );
      }

      const lastRun = await fetchNodeLastRun(applicationId!, nodeId!);
      if (lastRun?.flow_run?.id) {
        onResolveRunScope?.(lastRun.flow_run.id);
      }
      return lastRun;
    },
    enabled: Boolean(applicationId && nodeId)
  });
  if (lastRunQuery.isPending) {
    return <Result status="info" title={i18nText("agentFlow", "auto.loading_last_run")} />;
  }

  if (lastRunQuery.isError) {
    return <Result status="error" title={i18nText("agentFlow", "auto.last_run_failed_load")} />;
  }

  const emptyDescription = activeRunId
    ? i18nText("agentFlow", "auto.record_node_run")
    : i18nText("agentFlow", "auto.node_running_records_yet");

  if (!lastRunQuery.data) {
    return renderLastRunContent({
      schema,
      adapter,
      lastRun: null,
      emptyDescription
    });
  }

  if (!isNodeLastRun(lastRunQuery.data)) {
    return <Result status="warning" title={i18nText("agentFlow", "auto.abnormal_data_last_run")} />;
  }

  const lastRun = lastRunQuery.data;

  return renderLastRunContent({
    schema,
    adapter,
    lastRun,
    emptyDescription
  });
}
