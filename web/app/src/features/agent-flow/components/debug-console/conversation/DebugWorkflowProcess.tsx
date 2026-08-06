import { Think, ThoughtChain } from '@ant-design/x';
import type { ThoughtChainItemType } from '@ant-design/x';
import { useEffect, useMemo, useRef, useState } from 'react';
import { DownOutlined, RightOutlined } from '@ant-design/icons';
import { Typography } from 'antd';

import type { AgentFlowTraceItem } from '../../../api/runtime';
import type { RuntimeDebugArtifactBatchLoader } from '../../detail/last-run/runtime-debug-payload';
import { DebugWorkflowNodeRow, StatusIcon } from './DebugWorkflowNodeRow';
import { DebugWorkflowNodeDetailContent } from './LlmToolTraceTree';
import { groupTraceItemsForDisplay } from './debug-workflow-trace-utils';
import { collectLlmToolCallbacks } from './llm-tool-callbacks';
import { i18nText } from '../../../../../shared/i18n/text';
import { DebugMarkdownContent } from './DebugMarkdownContent';

function workflowStatus(items: AgentFlowTraceItem[]) {
  if (items.some((item) => item.status === 'failed')) {
    return 'failed';
  }

  if (items.some((item) => item.status === 'waiting_human')) {
    return 'waiting_human';
  }

  if (items.some((item) => item.status === 'waiting_callback')) {
    return 'waiting_callback';
  }

  if (items.some((item) => item.status === 'running')) {
    return 'running';
  }

  if (items.every((item) => item.status === 'succeeded')) {
    return 'succeeded';
  }

  return 'running';
}

function thoughtStatus(
  status: AgentFlowTraceItem['status']
): ThoughtChainItemType['status'] {
  switch (status) {
    case 'succeeded':
      return 'success';
    case 'failed':
      return 'error';
    case 'cancelled':
      return 'abort';
    default:
      return 'loading';
  }
}

export function DebugWorkflowProcess({
  items,
  reasoning,
  reasoningStreaming = false,
  onLoadArtifact,
  onLoadArtifacts
}: {
  items: AgentFlowTraceItem[];
  reasoning?: string;
  reasoningStreaming?: boolean;
  onLoadArtifact?: (artifactRef: string) => Promise<unknown>;
  onLoadArtifacts?: RuntimeDebugArtifactBatchLoader;
}) {
  const [expanded, setExpanded] = useState(true);
  const [expandedNodeKeys, setExpandedNodeKeys] = useState<Set<string>>(
    () => new Set()
  );
  const automaticallyExpandedNodeKeysRef = useRef(new Set<string>());
  const traceGroups = useMemo(() => groupTraceItemsForDisplay(items), [items]);
  const toolCallbackCounts = useMemo(
    () =>
      new Map(
        traceGroups.map((group) => [
          group.key,
          collectLlmToolCallbacks(group.item.debugPayload).length
        ])
      ),
    [traceGroups]
  );
  const automaticDetailKeys = useMemo(() => {
    const keys = traceGroups
      .filter((group) => (toolCallbackCounts.get(group.key) ?? 0) > 0)
      .map((group) => group.key);
    const reasoningLlmGroup = reasoning?.trim()
      ? [...traceGroups]
          .reverse()
          .find((group) => group.item.nodeType === 'llm')
      : null;

    if (reasoningLlmGroup && !keys.includes(reasoningLlmGroup.key)) {
      keys.push(reasoningLlmGroup.key);
    }

    return keys;
  }, [reasoning, toolCallbackCounts, traceGroups]);

  useEffect(() => {
    setExpandedNodeKeys((current) => {
      const next = new Set(current);
      let changed = false;

      automaticDetailKeys.forEach((key) => {
        if (automaticallyExpandedNodeKeysRef.current.has(key)) {
          return;
        }
        automaticallyExpandedNodeKeysRef.current.add(key);
        if (!next.has(key)) {
          next.add(key);
          changed = true;
        }
      });

      return changed ? next : current;
    });
  }, [automaticDetailKeys]);
  const thoughtItems = useMemo<ThoughtChainItemType[]>(() => {
    const lastLlmIndex = traceGroups.reduce(
      (lastIndex, group, index) =>
        group.item.nodeType === 'llm' ? index : lastIndex,
      -1
    );

    return traceGroups.map((group, index) => {
      const item = group.item;
      const showReasoning =
        item.nodeType === 'llm' &&
        index === lastLlmIndex &&
        Boolean(reasoning?.trim());

      return {
        key: group.key,
        icon: false,
        title: <DebugWorkflowNodeRow item={item} />,
        status: thoughtStatus(item.status),
        blink: item.status === 'running',
        collapsible: true,
        content: (
          <div className="agent-flow-editor__debug-workflow-node-detail">
            {showReasoning ? (
              <Think
                defaultExpanded={item.status === 'running'}
                loading={item.status === 'running'}
                title={i18nText('agentFlow', 'auto.think')}
                blink={reasoningStreaming && item.status === 'running'}
                className="agent-flow-editor__debug-workflow-think"
              >
                <DebugMarkdownContent
                  className="agent-flow-editor__debug-workflow-think-content"
                  content={reasoning ?? ''}
                  streaming={reasoningStreaming}
                />
              </Think>
            ) : null}
            <DebugWorkflowNodeDetailContent
              item={item}
              onLoadArtifact={onLoadArtifact}
              onLoadArtifacts={onLoadArtifacts}
            />
          </div>
        )
      };
    });
  }, [
    onLoadArtifact,
    onLoadArtifacts,
    reasoning,
    reasoningStreaming,
    toolCallbackCounts,
    traceGroups
  ]);

  if (items.length === 0) {
    return null;
  }

  const status = workflowStatus(items);

  return (
    <section
      aria-label={i18nText('agentFlow', 'auto.workflow')}
      className="agent-flow-editor__debug-workflow-process"
    >
      <button
        aria-expanded={expanded}
        className="agent-flow-editor__debug-workflow-header"
        onClick={() => setExpanded((current) => !current)}
        type="button"
      >
        <span className="agent-flow-editor__debug-workflow-title">
          <StatusIcon status={status} />
          <Typography.Text>
            {i18nText('agentFlow', 'auto.workflow')}
          </Typography.Text>
        </span>
        {expanded ? (
          <DownOutlined className="agent-flow-editor__debug-workflow-collapse" />
        ) : (
          <RightOutlined className="agent-flow-editor__debug-workflow-collapse" />
        )}
      </button>
      {expanded ? (
        <ThoughtChain
          classNames={{
            item: 'agent-flow-editor__debug-workflow-thought-item',
            itemContent: 'agent-flow-editor__debug-workflow-thought-content',
            itemHeader: 'agent-flow-editor__debug-workflow-thought-header'
          }}
          expandedKeys={[...expandedNodeKeys]}
          items={thoughtItems}
          line={false}
          rootClassName="agent-flow-editor__debug-workflow-thought-chain"
          onExpand={(keys) => setExpandedNodeKeys(new Set(keys.map(String)))}
        />
      ) : null}
    </section>
  );
}
